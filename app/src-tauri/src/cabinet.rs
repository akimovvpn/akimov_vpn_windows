use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};
use url::Url;

// These are deployment preferences, never panel/admin credentials.
#[derive(Deserialize, Serialize)]
struct Saved {
    base_url: String,
    cabinet_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    protected_refresh: Vec<u8>,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            base_url: "https://api.akimov.bid/api/v1".into(),
            cabinet_url: "https://akimov.bid".into(),
            protected_refresh: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    token_type: String,
}

struct State {
    saved: Saved,
    access: String,
    refresh: String,
    load_error: Option<String>,
}

pub struct Cabinet {
    dir: PathBuf,
    agent: Result<ureq::Agent, String>,
    // Local port of the running VPN core, 0 when it is not running. Used only as a fallback route.
    proxy: std::sync::atomic::AtomicU16,
    // The direct route failed and the VPN route worked: skip the direct attempt while the VPN is up.
    via_vpn: std::sync::atomic::AtomicBool,
    // Holding this lock through HTTP also serializes refresh and account changes.
    state: Mutex<State>,
}

// Internal markers for "the request never reached the cabinet", which makes a second route safe for any method.
const EDGE_BLOCKED: &str = "edge_blocked";
const UNSENT: &str = "unsent";
const UNREACHABLE: &str = "Cabinet connection failed. Check your internet and API address.";

/// The edge in front of the API rejects some networks before the cabinet sees the request.
/// Its answer names the caller's address; the cabinet's own 403 ("Banned") does not.
fn edge_blocked(status: u16, value: &Value) -> bool {
    status == 403 && value["error"] == "forbidden" && value["ip"].is_string()
}

fn build_agent(proxy_port: u16) -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|_| "Windows TLS is unavailable.")?;
    let mut builder = ureq::AgentBuilder::new()
        .tls_connector(Arc::new(tls))
        .redirects(0) // Never forward credentials to a redirect destination.
        .max_idle_connections(0) // Avoid ureq's automatic retries of stale pooled connections.
        .timeout_connect(Duration::from_secs(6))
        .timeout(Duration::from_secs(25));
    if proxy_port != 0 {
        let proxy = ureq::Proxy::new(format!("http://127.0.0.1:{proxy_port}"))
            .map_err(|_| "The VPN route is unavailable.")?;
        builder = builder.proxy(proxy);
    }
    Ok(builder.build())
}

impl Cabinet {
    pub fn new(dir: PathBuf) -> Self {
        let loaded = (|| -> Result<(Saved, String), String> {
            let bytes = match fs::read(dir.join("cabinet.json")) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok((Saved::default(), String::new()))
                }
                Err(_) => return Err("Could not read cabinet preferences.".into()),
            };
            if bytes.len() > 128 * 1024 {
                return Err("Cabinet preferences are invalid.".into());
            }
            let saved: Saved =
                serde_json::from_slice(&bytes).map_err(|_| "Cabinet preferences are invalid.")?;
            let fixed = Saved::default();
            if saved.base_url != fixed.base_url || saved.cabinet_url != fixed.cabinet_url {
                // Saved for an earlier account service: its session means nothing here. Start signed out.
                return Ok((fixed, String::new()));
            }
            if !saved.base_url.is_empty() {
                if origin(&saved.base_url, true)? != saved.base_url
                    || origin(&saved.cabinet_url, false)? != saved.cabinet_url
                {
                    return Err("Cabinet preferences are invalid.".into());
                }
            } else if !saved.protected_refresh.is_empty() {
                return Err("Cabinet session has no API origin.".into());
            }
            let refresh = if saved.protected_refresh.is_empty() {
                String::new()
            } else {
                let decrypted = protect(&saved.protected_refresh, false)?;
                let session: Value = serde_json::from_slice(&decrypted)
                    .map_err(|_| "Saved cabinet session is invalid.")?;
                if session["base_url"].as_str() != Some(saved.base_url.as_str()) {
                    return Err("Saved session belongs to another API origin.".into());
                }
                session["refresh_token"]
                    .as_str()
                    .filter(|s| valid_token(s))
                    .ok_or("Saved cabinet session is invalid.")?
                    .to_owned()
            };
            Ok((saved, refresh))
        })();
        let (saved, refresh, load_error) = match loaded {
            Ok((saved, refresh)) => (saved, refresh, None),
            Err(error) => (Saved::default(), String::new(), Some(error)),
        };
        let agent = build_agent(0);
        Self {
            dir,
            agent,
            proxy: std::sync::atomic::AtomicU16::new(0),
            via_vpn: std::sync::atomic::AtomicBool::new(false),
            state: Mutex::new(State {
                saved,
                access: String::new(),
                refresh,
                load_error,
            }),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, String> {
        self.state
            .lock()
            .map_err(|_| "Cabinet session is unavailable. Restart the app.".into())
    }

    pub fn require_session(&self) -> Result<(), String> {
        if self.lock()?.refresh.is_empty() {
            return Err("Please sign in again.".into());
        }
        Ok(())
    }

    /// Answers from what is saved on this computer, without the network: an unreachable
    /// account service must not hold the window on its startup screen.
    pub fn boot(&self) -> Result<Value, String> {
        let state = self.lock()?;
        Ok(json!({
            "configured": !state.saved.base_url.is_empty(),
            "signed_in": !state.refresh.is_empty(),
            "base_url": state.saved.base_url,
            "cabinet_url": state.saved.cabinet_url,
            "error": state.load_error,
        }))
    }

    /// Starts Telegram's official sign-in: the cabinet issues a one-time challenge and names the bot.
    pub fn telegram_challenge(&self) -> Result<TelegramChallenge, String> {
        const UNAVAILABLE: &str = "Telegram sign-in is unavailable right now.";
        let state = self.lock()?;
        let value = self
            .http(
                &state.saved.base_url,
                "POST",
                "/auth/telegram/oidc/request",
                None,
                None,
            )
            .map_err(|e| {
                if e.status == 0 {
                    e.message
                } else {
                    UNAVAILABLE.into()
                }
            })?;
        let text = |key: &str| match &value[key] {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        let (token, client, nonce) = (text("token"), text("client_id"), text("nonce"));
        if !valid_token(&token)
            || !valid_token(&nonce)
            || client.is_empty()
            || !client.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(UNAVAILABLE.into());
        }
        // The same request the cabinet page makes; Telegram accepts it only for the bot's own site.
        let site = &state.saved.cabinet_url;
        let mut url = Url::parse("https://oauth.telegram.org/auth").map_err(|_| UNAVAILABLE)?;
        url.query_pairs_mut()
            .append_pair("response_type", "post_message")
            .append_pair("client_id", &client)
            .append_pair("redirect_uri", &format!("{site}/"))
            .append_pair("origin", site)
            .append_pair("scope", "openid profile")
            .append_pair("nonce", &nonce)
            .append_pair("lang", "en");
        Ok(TelegramChallenge {
            token,
            url,
            expires: value["expires_in"].as_u64().unwrap_or(300).clamp(30, 600),
        })
    }

    /// `answer` is what Telegram's page hands back: an ID token, or the classic widget fields.
    /// Nothing in it is trusted here; the cabinet verifies it against the challenge.
    pub fn telegram_complete(&self, token: &str, answer: &Value) -> Result<Value, String> {
        const REFUSED: &str = "Telegram sign-in was not completed. Try again.";
        let id_token = answer.as_str().or_else(|| answer["id_token"].as_str());
        let body = match (id_token, answer.as_object()) {
            (Some(id), _) if valid_token(id) => json!({"token": token, "id_token": id}),
            (None, Some(fields)) if fields.contains_key("hash") && fields.len() <= 16 => {
                json!({"token": token, "widget": fields})
            }
            _ => return Err(REFUSED.into()),
        };
        let mut state = self.lock()?;
        let tokens = self
            .http(
                &state.saved.base_url,
                "POST",
                "/auth/telegram/oidc/complete",
                None,
                Some(&body),
            )
            .map_err(|e| match e.status {
                0 | 403 | 429 => e.message,
                _ => REFUSED.into(),
            })?;
        let tokens = parse_tokens(tokens)?;
        // Verify the account before replacing any existing session.
        let me = self
            .http(
                &state.saved.base_url,
                "GET",
                "/me",
                Some(&tokens.access_token),
                None,
            )
            .map_err(|e| e.message)?;
        validate_me(&me)?;
        self.set_tokens(&mut state, tokens)?;
        Ok(me)
    }

    pub fn login(&self, email: &str, password: &str) -> Result<Value, String> {
        let email = email.trim();
        if email.is_empty()
            || email.len() > 320
            || !email.contains('@')
            || password.is_empty()
            || password.len() > 512
        {
            return Err("Enter a valid email and password.".into());
        }
        let mut state = self.lock()?;
        let tokens = self
            .http(
                &state.saved.base_url,
                "POST",
                "/auth/login",
                None,
                Some(&json!({"email": email, "password": password})),
            )
            .map_err(|e| {
                if e.status == 401 {
                    "Wrong email or password.".into()
                } else {
                    e.message
                }
            })?;
        let tokens = parse_tokens(tokens)?;
        // Verify the account before replacing any existing session.
        let me = self
            .http(
                &state.saved.base_url,
                "GET",
                "/me",
                Some(&tokens.access_token),
                None,
            )
            .map_err(|e| e.message)?;
        validate_me(&me)?;
        self.set_tokens(&mut state, tokens)?;
        Ok(me)
    }

    pub fn auth(&self, action: &str, body: Value) -> Result<Value, String> {
        let email = body["email"].as_str().unwrap_or("").trim();
        let password = body["password"].as_str().unwrap_or("");
        let path = match action {
            "register" => "/auth/register",
            "verify" => "/auth/verify",
            "resend" => "/auth/resend",
            "forgot" => "/auth/password/forgot",
            "reset" => "/auth/password/reset",
            _ => return Err("Something went wrong. Try again later.".into()),
        };
        if action != "reset" && (email.len() > 320 || !email.contains('@')) {
            return Err("Enter a valid email address.".into());
        }
        if ["register", "reset"].contains(&action) && !(8..=128).contains(&password.chars().count())
        {
            return Err("Use a password between 8 and 128 characters.".into());
        }
        if action == "register"
            && (body["accept_terms"] != true || body["accept_personal_data"] != true)
        {
            return Err("Please accept both agreements to create an account.".into());
        }
        if action == "verify"
            && !body["code"]
                .as_str()
                .is_some_and(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("Enter the six-digit code from your email.".into());
        }
        if action == "reset"
            && !body["token"].as_str().is_some_and(|s| {
                !s.is_empty()
                    && s.len() <= 2048
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            })
        {
            return Err("Paste a valid reset link from your email.".into());
        }
        let mut state = self.lock()?;
        let result = self
            .http(&state.saved.base_url, "POST", path, None, Some(&body))
            .map_err(|e| e.message)?;
        if result.get("access_token").is_none() {
            if ["verify", "reset"].contains(&action) {
                return Err("Something went wrong. Try again later.".into());
            }
            return Ok(result);
        }
        let tokens = parse_tokens(result)?;
        let me = self
            .http(
                &state.saved.base_url,
                "GET",
                "/me",
                Some(&tokens.access_token),
                None,
            )
            .map_err(|e| e.message)?;
        validate_me(&me)?;
        self.set_tokens(&mut state, tokens)?;
        Ok(json!({"status":"signed_in","me":me}))
    }

    pub fn logout(&self) -> Result<Value, String> {
        let mut state = self.lock()?;
        self.clear_session(&mut state)?;
        Ok(json!({"ok": true}))
    }

    pub fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
        let method = method.to_ascii_uppercase();
        validate_request(&method, path, body.as_ref())?;
        let mut state = self.lock()?;
        if method == "GET" && path == "/legal" {
            return self
                .http(&state.saved.base_url, "GET", "/legal", None, None)
                .map_err(|e| e.message);
        }
        self.authenticated(&mut state, &method, path, body.as_ref())
    }

    fn authenticated(
        &self,
        state: &mut State,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, String> {
        if state.refresh.is_empty() {
            return Err("Sign in to your cabinet first.".into());
        }
        if state.access.is_empty() {
            self.refresh(state)?;
        }
        match self.http(
            &state.saved.base_url,
            method,
            path,
            Some(&state.access),
            body,
        ) {
            Ok(value) => Ok(value),
            Err(error) if error.status == 401 => {
                self.refresh(state)?;
                match self.http(
                    &state.saved.base_url,
                    method,
                    path,
                    Some(&state.access),
                    body,
                ) {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        if error.status == 401 {
                            self.clear_session(state)?;
                        }
                        Err(error.message)
                    }
                }
            }
            Err(error) => Err(error.message),
        }
    }

    fn refresh(&self, state: &mut State) -> Result<(), String> {
        match self.http(
            &state.saved.base_url,
            "POST",
            "/auth/refresh",
            None,
            Some(&json!({"refresh_token": state.refresh})),
        ) {
            Ok(value) => self.set_tokens(state, parse_tokens(value)?),
            Err(error) => {
                if error.status == 401 {
                    self.clear_session(state)?;
                }
                Err(error.message)
            }
        }
    }

    fn set_tokens(&self, state: &mut State, tokens: Tokens) -> Result<(), String> {
        let session = serde_json::to_vec(
            &json!({"base_url": state.saved.base_url, "refresh_token": tokens.refresh_token}),
        )
        .map_err(|_| "Could not encode cabinet session.")?;
        let encrypted = protect(&session, true)?;
        let saved = Saved {
            base_url: state.saved.base_url.clone(),
            cabinet_url: state.saved.cabinet_url.clone(),
            protected_refresh: encrypted,
        };
        self.save(&saved)?;
        state.saved = saved;
        state.access = tokens.access_token;
        state.refresh = tokens.refresh_token;
        state.load_error = None;
        Ok(())
    }

    fn clear_session(&self, state: &mut State) -> Result<(), String> {
        state.access.clear();
        state.refresh.clear();
        state.saved.protected_refresh.clear();
        self.save(&state.saved)
    }

    fn save(&self, saved: &Saved) -> Result<(), String> {
        fs::create_dir_all(&self.dir)
            .map_err(|_| "Could not create cabinet preferences directory.")?;
        let path = self.dir.join("cabinet.json");
        let temporary = self.dir.join(format!("cabinet.{}.tmp", std::process::id()));
        let bytes =
            serde_json::to_vec(saved).map_err(|_| "Could not encode cabinet preferences.")?;
        let write = (|| -> std::io::Result<()> {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &path)
        })();
        if write.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write.map_err(|_| "Could not save cabinet preferences securely.".into())
    }

    /// Tell the cabinet client where the running VPN core listens (0 when it is not running).
    pub fn set_proxy(&self, port: u16) {
        self.proxy.store(port, std::sync::atomic::Ordering::Relaxed);
    }

    /// Direct first. If the request provably never reached the cabinet and the VPN is up, go through the VPN once.
    fn http(
        &self,
        base: &str,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<&Value>,
    ) -> Result<Value, HttpError> {
        use std::sync::atomic::Ordering::Relaxed;
        let unsent = |e: &HttpError| e.message == EDGE_BLOCKED || e.message == UNSENT;
        let agent = self.agent.as_ref().map_err(|e| HttpError::local(e))?;
        let port = self.proxy.load(Relaxed);
        let vpn_first = port != 0 && self.via_vpn.load(Relaxed);
        let direct = if vpn_first {
            Err(HttpError::local(UNSENT))
        } else {
            self.http_via(agent, base, method, path, token, body)
        };
        let result = match direct {
            Err(e) if port != 0 && unsent(&e) => {
                let via_vpn = build_agent(port).map_err(|e| HttpError::local(&e))?;
                let answer = self.http_via(&via_vpn, base, method, path, token, body);
                // A direct attempt costs a full connect timeout; remember which route answers.
                self.via_vpn
                    .store(!matches!(&answer, Err(e) if unsent(e)), Relaxed);
                answer
            }
            other => other,
        };
        result.map_err(|e| {
            if e.message == EDGE_BLOCKED || e.message == UNSENT {
                HttpError::local(UNREACHABLE)
            } else {
                e
            }
        })
    }

    fn http_via(
        &self,
        agent: &ureq::Agent,
        base: &str,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<&Value>,
    ) -> Result<Value, HttpError> {
        if base.is_empty() {
            return Err(HttpError::local("Set your cabinet API address first."));
        }
        let tunnel = matches!(method, "PATCH" | "DELETE");
        let mut request = agent
            .request(
                if tunnel { "POST" } else { method },
                &format!("{base}{path}"),
            )
            .set("Accept", "application/json")
            .set("X-Cabinet-Client", "web");
        if tunnel {
            request = request.set("X-Cabinet-Verb", method);
        }
        if let Some(token) = token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        let result = match body {
            Some(body) => request.send_json(body),
            None if method == "POST" || method == "PATCH" => request.send_json(json!({})),
            None => request.call(),
        };
        let (response, status) = match result {
            Ok(response) => {
                let status = response.status();
                (response, status)
            }
            Err(ureq::Error::Status(status, response)) => (response, status),
            // Transport diagnostics may include a URL; keep tokens/links out of errors.
            Err(ureq::Error::Transport(transport)) => {
                // Only a failure to resolve or connect proves nothing was sent.
                let unsent = matches!(
                    transport.kind(),
                    ureq::ErrorKind::Dns | ureq::ErrorKind::ConnectionFailed
                );
                return Err(HttpError::local(if unsent { UNSENT } else { UNREACHABLE }));
            }
        };
        let mut bytes = Vec::new();
        response.into_reader().take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)
            .map_err(|_| HttpError { status, message: "Cabinet response could not be read. The operation may have completed; refresh before retrying.".into() })?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(HttpError {
                status,
                message: "Cabinet response is too large.".into(),
            });
        }
        let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if edge_blocked(status, &value) {
            return Err(HttpError::local(EDGE_BLOCKED));
        }
        if !(200..300).contains(&status) {
            return Err(api_error(status, &value));
        }
        if value.is_null() {
            return Err(HttpError { status, message: "Cabinet returned an invalid JSON response. Refresh before retrying an operation.".into() });
        }
        Ok(value)
    }
}

pub struct TelegramChallenge {
    pub token: String,
    pub url: Url,
    pub expires: u64,
}

struct HttpError {
    status: u16,
    message: String,
}
impl HttpError {
    fn local(message: &str) -> Self {
        Self {
            status: 0,
            message: message.into(),
        }
    }
}

fn api_error(status: u16, value: &Value) -> HttpError {
    let fallback = match status {
        401 => "Please sign in again.",
        402 => "Insufficient balance.",
        403 => "Access to this account is restricted.",
        429 => "Too many requests. Please wait and try again.",
        409 => "This email is already registered. Try signing in.",
        410 => "This code or reset link has expired. Request a new one.",
        400 => "Check the details and try again.",
        _ => "Something went wrong. Try again later.",
    };
    let raw = value["error"].as_str().unwrap_or("").to_lowercase();
    let message = if status == 400 && (raw.contains("код") || raw.contains("code")) {
        "That code is incorrect. Try again."
    } else {
        fallback
    };
    HttpError {
        status,
        message: message.to_string(),
    }
}

fn valid_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 16384
        && token
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

fn parse_tokens(value: Value) -> Result<Tokens, String> {
    let tokens: Tokens = serde_json::from_value(value)
        .map_err(|_| "Cabinet returned an invalid sign-in response.")?;
    if tokens.token_type != "bearer"
        || !valid_token(&tokens.access_token)
        || !valid_token(&tokens.refresh_token)
    {
        return Err("Cabinet returned an invalid sign-in response.".into());
    }
    Ok(tokens)
}

fn validate_me(value: &Value) -> Result<(), String> {
    if value["id"].as_i64().is_none()
        || !value["subscriptions"].is_array()
        || value["balance"].as_i64().is_none()
    {
        return Err("Cabinet returned an invalid account response.".into());
    }
    Ok(())
}

fn origin(input: &str, api: bool) -> Result<String, String> {
    let input = input.trim();
    let error =
        "Use an HTTPS origin, for example https://cabinet.example.com (API may end with /api/v1).";
    if input.len() > 2048 || input.contains('\\') || input.chars().any(|c| c.is_control()) {
        return Err(error.into());
    }
    let mut url = Url::parse(input).map_err(|_| error)?;
    let authority = input
        .strip_prefix("https://")
        .and_then(|s| s.split('/').next())
        .ok_or(error)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || authority.contains('@')
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
        || !(url.path() == "/" || api && matches!(url.path(), "/api/v1" | "/api/v1/"))
    {
        return Err(error.into());
    }
    // Reject paths URL parsing would otherwise normalize (e.g. /admin/../).
    let raw_path = input
        .strip_prefix("https://")
        .and_then(|s| s.find('/').map(|i| &s[i..]))
        .unwrap_or("");
    if !matches!(raw_path, "" | "/") && !(api && matches!(raw_path, "/api/v1" | "/api/v1/")) {
        return Err(error.into());
    }
    url.set_path(if api { "/api/v1" } else { "/" });
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

fn positive_id(s: &str) -> bool {
    !s.starts_with('0')
        && s.bytes().all(|c| c.is_ascii_digit())
        && s.parse::<u64>()
            .map(|n| n > 0 && n <= i64::MAX as u64)
            .unwrap_or(false)
}

fn uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

fn validate_request(method: &str, path: &str, body: Option<&Value>) -> Result<(), String> {
    let error = "This cabinet endpoint or request is not supported.";
    if path.len() > 1024
        || !path.is_ascii()
        || path.bytes().any(|c| c.is_ascii_control())
        || path.contains(['#', '\\'])
        || path.contains("//")
    {
        return Err(error.into());
    }
    let (route, query) = path
        .split_once('?')
        .map_or((path, None), |(p, q)| (p, Some(q)));
    let parts: Vec<&str> = route.split('/').collect();
    let sub_query = matches!(
        route,
        "/devices" | "/subscription/link" | "/subscription/connect-info"
    ) || parts.len() == 3 && parts[1] == "devices";
    if let Some(query) = query {
        if !sub_query
            || !query
                .strip_prefix("sub_id=")
                .map(positive_id)
                .unwrap_or(false)
        {
            return Err(error.into());
        }
    }
    let subscription = parts.len() >= 3 && parts[1] == "subscription" && positive_id(parts[2]);
    let checkout = method == "POST"
        && (route == "/buy"
            || subscription
                && parts.len() == 4
                && matches!(parts[3], "renew" | "extra-devices" | "extra-gb"));
    let device = parts.len() == 3 && parts[1] == "devices" && {
        let hwid = parts[2].replace("%3D", "=").replace("%3d", "=");
        (10..=64).contains(&hwid.len())
            && hwid
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'=' | b'-'))
    };
    let allowed = match method {
        "GET" => {
            matches!(
                route,
                "/me"
                    | "/legal"
                    | "/me/traffic"
                    | "/me/transactions"
                    | "/tariffs"
                    | "/trial"
                    | "/devices"
                    | "/subscription/link"
                    | "/subscription/connect-info"
                    | "/notifications"
                    | "/referral"
                    | "/referral/users"
            ) || parts.len() == 3 && parts[1] == "checkout" && uuid(parts[2])
        }
        "POST" => {
            checkout
                || matches!(
                    route,
                    "/trial" | "/topup" | "/promo" | "/me/notifications" | "/notifications/read"
                )
                || subscription && parts.len() == 4 && parts[3] == "auto-renew"
        }
        "PATCH" => device,
        "DELETE" => device || route == "/notifications" || subscription && parts.len() == 3,
        _ => false,
    };
    if !allowed {
        return Err(error.into());
    }
    if let Some(body) = body {
        if !body.is_object()
            || matches!(method, "GET" | "DELETE")
            || serde_json::to_vec(body).map_err(|_| error)?.len() > 65536
        {
            return Err(error.into());
        }
    }
    if checkout
        && !body
            .and_then(|b| b["request_id"].as_str())
            .map(uuid)
            .unwrap_or(false)
    {
        return Err("Checkout requires a request_id UUID. Keep the same ID when retrying an unfinished payment.".into());
    }
    if method == "PATCH"
        && !body
            .and_then(|b| b["name"].as_str())
            .map(|name| !name.trim().is_empty() && name.chars().count() <= 64)
            .unwrap_or(false)
    {
        return Err("Device name must contain 1 to 64 characters.".into());
    }
    Ok(())
}

#[cfg(windows)]
fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes
            .len()
            .try_into()
            .map_err(|_| "Cabinet session is too large.")?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        let ok = if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(
                "Windows could not secure or unlock the saved cabinet session. Sign in again."
                    .into(),
            );
        }
        let result = if output.cbData == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        LocalFree(output.pbData.cast());
        Ok(result)
    }
}

#[cfg(not(windows))]
fn protect(_: &[u8], _: bool) -> Result<Vec<u8>, String> {
    Err("Cabinet session storage requires Windows DPAPI.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("akimov-cabinet-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_session_saved_for_another_service_starts_signed_out_without_an_error() {
        let dir = scratch("moved");
        fs::write(
            dir.join("cabinet.json"),
            r#"{"base_url":"https://api.example.com/api/v1","cabinet_url":"https://cabinet.example.com","protected_refresh":[1,2,3]}"#,
        )
        .unwrap();
        let boot = Cabinet::new(dir.clone()).boot().unwrap();
        assert_eq!(boot["signed_in"], false);
        assert!(boot["error"].is_null());
        assert_eq!(boot["base_url"], Saved::default().base_url);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn telegram_answers_that_are_not_a_token_or_widget_fields_never_reach_the_cabinet() {
        let dir = scratch("telegram");
        let cabinet = Cabinet::new(dir.clone());
        for junk in [
            json!(null),
            json!(42),
            json!(""),
            json!("has spaces"),
            json!({"id": 1}),
            json!({"id_token": "bad token"}),
        ] {
            let error = cabinet.telegram_complete("challenge", &junk).unwrap_err();
            assert!(
                error.starts_with("Telegram sign-in was not completed"),
                "{junk}"
            );
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn only_the_edge_refusal_counts_as_blocked() {
        assert!(edge_blocked(
            403,
            &json!({"error": "forbidden", "ip": "203.0.113.7"})
        ));
        // The cabinet's own refusals must never be retried through another route.
        assert!(!edge_blocked(403, &json!({"error": "Banned"})));
        assert!(!edge_blocked(403, &json!({"error": "forbidden"})));
        assert!(!edge_blocked(
            401,
            &json!({"error": "forbidden", "ip": "203.0.113.7"})
        ));
        assert!(!edge_blocked(403, &Value::Null));
    }

    #[test]
    fn origins_allow_only_https_without_credentials_or_paths() {
        assert_eq!(
            origin("https://cabinet.example.com/", true).unwrap(),
            "https://cabinet.example.com/api/v1"
        );
        assert_eq!(
            origin("https://api.example.com/api/v1/", true).unwrap(),
            "https://api.example.com/api/v1"
        );
        for bad in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://@example.com",
            "https://example.com?x=1",
            "https://example.com/#fragment",
            "https://example.com/admin",
            "https://example.com/admin/../",
            "https://example.com\\@evil.test",
            "https://example.com/%2e%2e/",
        ] {
            assert!(origin(bad, true).is_err(), "{bad}");
        }
    }

    #[test]
    fn whitelist_excludes_admin_traversal_and_unimplemented_routes() {
        assert!(validate_request("GET", "/legal", None).is_ok());
        assert!(validate_request("POST", "/legal", None).is_err());
        assert!(validate_request("GET", "/me/transactions", None).is_ok());
        assert!(validate_request("GET", "/devices?sub_id=42", None).is_ok());
        assert!(validate_request("DELETE", "/devices/abcdefghij%3D?sub_id=42", None).is_ok());
        for bad in [
            "/admin/users",
            "/api/v1/me",
            "//evil.test/me",
            "/devices/../../admin/users",
            "/devices/%2e%2e",
            "/me?url=https://evil.test",
            "/devices?sub_id=1&sub_id=2",
            "/devices?sub_id=-1",
            "/devices?sub_id=1#fragment",
            "/app/config",
            "/checkout/quote",
            "/auth/login",
            "/subscription/1/../2",
        ] {
            assert!(validate_request("GET", bad, None).is_err(), "{bad}");
        }
        assert!(validate_request("POST", "/buy", Some(&json!({"tariff_id": 1}))).is_err());
        assert!(validate_request(
            "POST",
            "/buy",
            Some(&json!({"request_id":"fa438c74-c00f-493c-9e23-86748a899047","tariff_id":1}))
        )
        .is_ok());
        assert!(validate_request(
            "PATCH",
            "/devices/abcdefghij",
            Some(&json!({"name":"Laptop"}))
        )
        .is_ok());
    }

    #[test]
    fn auth_shapes_and_public_error_envelopes_are_checked() {
        assert!(parse_tokens(
            json!({"access_token":"a.b.c","refresh_token":"d.e.f","token_type":"bearer"})
        )
        .is_ok());
        assert!(parse_tokens(json!({"access_token":"a.b.c","refresh_token":"d.e.f"})).is_err());
        assert!(parse_tokens(
            json!({"access_token":"a\r\nb","refresh_token":"d","token_type":"bearer"})
        )
        .is_err());
        assert!(validate_me(&json!({"id":-2,"subscriptions":[],"balance":12300})).is_ok());
        assert_eq!(
            api_error(402, &json!({"error":"Недостаточно средств"})).message,
            "Insufficient balance."
        );
        assert_eq!(
            api_error(401, &Value::Null).message,
            "Please sign in again."
        );
        assert_eq!(
            api_error(503, &json!({"error":"database failed, internal exception"})).message,
            "Something went wrong. Try again later."
        );
    }

    #[test]
    fn public_auth_rejects_invalid_input_before_sending_any_request() {
        let cabinet = Cabinet::new(std::env::temp_dir().join("akimov-auth-validation-only"));
        assert!(cabinet.auth("register",json!({"email":"test@example.invalid","password":"valid-password","accept_terms":true,"accept_personal_data":false})).unwrap_err().contains("agreements"));
        assert!(cabinet
            .auth(
                "verify",
                json!({"email":"test@example.invalid","code":"12"})
            )
            .unwrap_err()
            .contains("six-digit"));
        assert!(cabinet
            .auth(
                "reset",
                json!({"token":"bad token","password":"valid-password"})
            )
            .unwrap_err()
            .contains("reset link"));
        assert!(cabinet.auth("admin", Value::Null).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_encrypts_with_current_windows_account() {
        let input = b"refresh-token-test-only";
        let encrypted = protect(input, true).unwrap();
        assert_ne!(encrypted, input);
        assert_eq!(protect(&encrypted, false).unwrap(), input);
    }

    #[cfg(windows)]
    #[test]
    fn saved_sessions_restore_without_exposing_tokens_and_logout_removes_them() {
        let dir = std::env::temp_dir().join(format!(
            "moonlight-cabinet-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cabinet = Cabinet::new(dir.clone());
        assert!(cabinet.require_session().is_err());
        cabinet
            .set_tokens(
                &mut cabinet.lock().unwrap(),
                Tokens {
                    access_token: "private-access-value".into(),
                    refresh_token: "private-refresh-value".into(),
                    token_type: "bearer".into(),
                },
            )
            .unwrap();
        let saved = fs::read_to_string(dir.join("cabinet.json")).unwrap();
        assert!(!saved.contains("private-access-value"));
        assert!(!saved.contains("private-refresh-value"));
        let restored = Cabinet::new(dir.clone());
        assert!(restored.require_session().is_ok());
        assert_eq!(restored.lock().unwrap().refresh, "private-refresh-value");
        assert!(restored.lock().unwrap().access.is_empty());
        restored.logout().unwrap();
        assert!(!fs::read_to_string(dir.join("cabinet.json"))
            .unwrap()
            .contains("protected_refresh"));
        assert!(Cabinet::new(dir.clone()).lock().unwrap().refresh.is_empty());
        let altered = Saved {
            base_url: "https://other.example/api/v1".into(),
            ..Saved::default()
        };
        fs::write(
            dir.join("cabinet.json"),
            serde_json::to_vec(&altered).unwrap(),
        )
        .unwrap();
        let locked = Cabinet::new(dir.clone());
        assert_eq!(
            locked.lock().unwrap().saved.base_url,
            Saved::default().base_url
        );
        // A file saved for another account service starts clean rather than failing.
        assert!(locked.lock().unwrap().load_error.is_none());
        assert!(locked.lock().unwrap().refresh.is_empty());
        fs::remove_file(dir.join("cabinet.json")).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
