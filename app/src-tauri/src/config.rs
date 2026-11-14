//! config = build(base, subscription, user rules). Nothing is edited in place, so user rules survive a refresh.
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

#[derive(Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: u64,
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    pub target: String,
    pub on: bool,
}

/// Only these keys are taken from a subscription. Ports, controller, DNS and TUN stay ours.
const KEEP: [&str; 6] = [
    "proxies",
    "proxy-groups",
    "proxy-providers",
    "rules",
    "rule-providers",
    "sub-rules",
];
const SYSTEM_RULES: [&str; 4] = [
    "DOMAIN-SUFFIX,local,DIRECT",
    "IP-CIDR,127.0.0.0/8,DIRECT,no-resolve",
    "IP-CIDR,192.168.0.0/16,DIRECT,no-resolve",
    "IP-CIDR,10.0.0.0/8,DIRECT,no-resolve",
];

pub struct Built {
    pub yaml: String,
    pub main: String,
    pub nodes: usize,
}

pub fn parse(text: &str) -> Result<Value, String> {
    let v: Value = serde_yaml::from_str(text)
        .map_err(|_| "This link did not return a mihomo profile.".to_string())?;
    let n = v
        .get("proxies")
        .and_then(Value::as_sequence)
        .map(|s| s.len())
        .unwrap_or(0);
    let p = v
        .get("proxy-providers")
        .and_then(Value::as_mapping)
        .map(|m| m.len())
        .unwrap_or(0);
    if n + p == 0 {
        return Err("This link has no servers in it.".into());
    }
    Ok(v)
}

fn names(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_sequence)
        .map(|s| {
            s.iter()
                .filter_map(|p| p.get("name").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// The group that node selection drives: the target of MATCH, else the first select group, else one we make.
fn main_group(sub: &Value, out: &mut Mapping) -> String {
    let mut all = out
        .get("proxy-groups")
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let target = sub
        .get("rules")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .rev()
        .find_map(|r| r.strip_prefix("MATCH,").and_then(|s| s.split(',').next()))
        .map(str::trim);
    let selector = all
        .iter()
        .find(|g| {
            g["type"].as_str() == Some("select")
                && target.is_none_or(|t| g["name"].as_str() == Some(t))
        })
        .and_then(|g| g["name"].as_str());
    if let Some(name) = selector {
        return name.to_string();
    }
    let mut name = "akimov".to_string();
    let used: Vec<_> = names(sub, "proxies")
        .into_iter()
        .chain(names(sub, "proxy-groups"))
        .collect();
    while used.contains(&name) {
        name.push('_');
    }
    let mut nodes = names(sub, "proxies");
    if let Some(target) = target {
        nodes.retain(|n| n != target);
        nodes.insert(0, target.to_string());
        if let Some(rules) = out.get_mut("rules").and_then(Value::as_sequence_mut) {
            for r in rules {
                if r.as_str().is_some_and(|r| r.starts_with("MATCH,")) {
                    *r = format!("MATCH,{name}").into();
                }
            }
        }
    }
    let mut g = Mapping::new();
    g.insert("name".into(), name.clone().into());
    g.insert("type".into(), "select".into());
    g.insert(
        "proxies".into(),
        Value::Sequence(nodes.into_iter().map(Value::from).collect()),
    );
    if let Some(providers) = sub.get("proxy-providers").and_then(Value::as_mapping) {
        g.insert(
            "use".into(),
            Value::Sequence(providers.keys().cloned().collect()),
        );
    }
    all.insert(0, Value::Mapping(g));
    out.insert("proxy-groups".into(), Value::Sequence(all));
    name
}

fn line(r: &Rule, main: &str) -> String {
    let target = match r.target.as_str() {
        "@direct" => "DIRECT",
        "@block" => "REJECT",
        _ => main,
    };
    let tail = if r.kind.starts_with("IP-CIDR") {
        ",no-resolve"
    } else {
        ""
    };
    format!("{},{},{}{}", r.kind, r.value, target, tail)
}

fn order_main_group(out: &mut Mapping, main: &str) {
    use std::collections::{HashMap, HashSet};
    let positions: HashMap<String, usize> = out
        .get("proxies")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, p)| p["name"].as_str().map(|n| (n.to_string(), i)))
        .collect();
    let groups: HashMap<String, Vec<String>> = out
        .get("proxy-groups")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(|g| {
            Some((
                g["name"].as_str()?.to_string(),
                g["proxies"]
                    .as_sequence()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
            ))
        })
        .collect();
    fn rank(
        name: &str,
        positions: &HashMap<String, usize>,
        groups: &HashMap<String, Vec<String>>,
        seen: &mut HashSet<String>,
    ) -> usize {
        if let Some(i) = positions.get(name) {
            return *i;
        }
        if !seen.insert(name.to_string()) {
            return usize::MAX;
        }
        let result = groups
            .get(name)
            .into_iter()
            .flatten()
            .map(|child| rank(child, positions, groups, seen))
            .min()
            .unwrap_or(usize::MAX);
        seen.remove(name);
        result
    }
    if let Some(groups_out) = out.get_mut("proxy-groups").and_then(Value::as_sequence_mut) {
        if let Some(group) = groups_out
            .iter_mut()
            .find(|g| g["name"].as_str() == Some(main))
        {
            if let Some(items) = group.get_mut("proxies").and_then(Value::as_sequence_mut) {
                // The subscription's first choice is its default automatic group.
                let tail = items.len().min(1);
                items[tail..].sort_by_key(|item| {
                    rank(
                        item.as_str().unwrap_or(""),
                        &positions,
                        &groups,
                        &mut HashSet::new(),
                    )
                });
            }
        }
    }
}

// Resolve the core's include-all list for the disconnected picker without starting a tunnel.
pub fn picker_profile(yaml: &str, main: &str) -> Result<Value, String> {
    let mut profile: Value = serde_yaml::from_str(yaml).map_err(|e| e.to_string())?;
    let proxies: Vec<_> = profile["proxies"]
        .as_sequence()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            Some((
                p["name"].as_str()?.to_string(),
                p["type"].as_str().unwrap_or("").to_string(),
            ))
        })
        .collect();
    fn patterns(value: &Value) -> Result<Vec<regex::Regex>, String> {
        value.as_str().unwrap_or("").split('`').filter(|p| !p.is_empty())
            .map(|p| regex::Regex::new(p).map_err(|_| "This subscription filter cannot be read. Update the subscription and try again.".into())).collect()
    }
    if let Some(groups) = profile["proxy-groups"].as_sequence_mut() {
        for group in groups {
            let filters = patterns(&group["filter"])?;
            let excludes = patterns(&group["exclude-filter"])?;
            let mut items = group["proxies"].as_sequence().cloned().unwrap_or_default();
            if group["include-all"].as_bool() == Some(true)
                || group["include-all-proxies"].as_bool() == Some(true)
            {
                items.extend(
                    proxies
                        .iter()
                        .filter(|(n, _)| {
                            filters.is_empty() || filters.iter().any(|f| f.is_match(n))
                        })
                        .map(|(n, _)| Value::from(n.clone())),
                );
            }
            let excluded_types = group["exclude-type"].as_str().unwrap_or("").to_string();
            let mut seen = std::collections::HashSet::new();
            items.retain(|item| {
                item.as_str().is_some_and(|n| {
                    seen.insert(n.to_string())
                        && !excludes.iter().any(|f| f.is_match(n))
                        && !proxies.iter().any(|(name, kind)| {
                            name == n
                                && excluded_types
                                    .split('|')
                                    .any(|t| !t.is_empty() && t.eq_ignore_ascii_case(kind))
                        })
                })
            });
            group["proxies"] = Value::Sequence(items);
        }
    }
    order_main_group(
        profile.as_mapping_mut().ok_or("Invalid subscription.")?,
        main,
    );
    Ok(profile)
}

pub fn build(
    sub: &Value,
    rules: &[Rule],
    mixed: u16,
    ctl: u16,
    secret: &str,
    tun: bool,
) -> Result<Built, String> {
    for r in rules {
        if !["@proxy", "@direct", "@block"].contains(&r.target.as_str())
            || r.value.trim().is_empty()
            || r.value.contains([',', '\n', '\r'])
        {
            return Err("Invalid rule value or target.".into());
        }
        let valid = match r.kind.as_str() {
            "DOMAIN" | "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" | "PROCESS-NAME" | "GEOSITE" => {
                !r.value.chars().any(char::is_whitespace)
            }
            "IP-CIDR" | "IP-CIDR6" => r
                .value
                .split_once('/')
                .map(|(ip, mask)| {
                    ip.parse::<std::net::IpAddr>()
                        .ok()
                        .zip(mask.parse::<u8>().ok())
                        .map(|(ip, mask)| {
                            if r.kind == "IP-CIDR" {
                                ip.is_ipv4() && mask <= 32
                            } else {
                                ip.is_ipv6() && mask <= 128
                            }
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false),
            "DST-PORT" => r.value.parse::<u16>().map(|p| p > 0).unwrap_or(false),
            _ => false,
        };
        if !valid {
            return Err(format!("Invalid {} rule.", r.kind));
        }
    }
    let mut out = Mapping::new();
    for k in KEEP {
        if let Some(v) = sub.get(k) {
            out.insert(k.into(), v.clone());
        }
    }
    for key in ["proxy-providers", "rule-providers"] {
        if let Some(providers) = out.get_mut(key).and_then(Value::as_mapping_mut) {
            for (i, (_, provider)) in providers.iter_mut().enumerate() {
                let p = provider.as_mapping_mut().ok_or("Invalid provider.")?;
                if p.get("type").and_then(Value::as_str) == Some("file") {
                    return Err("File providers are not supported in subscription links.".into());
                }
                p.insert("path".into(), format!("./providers/{key}-{i}.yaml").into());
            }
        }
    }
    let main = main_group(sub, &mut out);
    order_main_group(&mut out, &main);

    // order: system, the user's, the subscription's, then a final MATCH
    let mut all: Vec<Value> = SYSTEM_RULES.iter().map(|s| Value::from(*s)).collect();
    all.extend(
        rules
            .iter()
            .filter(|r| r.on)
            .map(|r| Value::from(line(r, &main))),
    );
    let theirs = out
        .get("rules")
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let has_match = theirs
        .iter()
        .any(|r| r.as_str().map(|s| s.starts_with("MATCH,")).unwrap_or(false));
    all.extend(theirs);
    if !has_match {
        all.push(Value::from(format!("MATCH,{main}")));
    }
    out.insert("rules".into(), Value::Sequence(all));

    out.insert("mixed-port".into(), Value::from(mixed));
    out.insert("allow-lan".into(), Value::from(false));
    out.insert("bind-address".into(), "127.0.0.1".into());
    out.insert("mode".into(), "rule".into());
    out.insert("log-level".into(), "warning".into());
    out.insert("ipv6".into(), Value::from(tun));
    out.insert("unified-delay".into(), Value::from(true));
    out.insert("tcp-concurrent".into(), Value::from(true));
    out.insert("find-process-mode".into(), "strict".into());
    out.insert(
        "external-controller".into(),
        Value::from(format!("127.0.0.1:{ctl}")),
    );
    out.insert("secret".into(), secret.into());
    let mut profile = Mapping::new();
    profile.insert("store-selected".into(), Value::from(true));
    out.insert("profile".into(), Value::Mapping(profile));
    let mut dns: Value = serde_yaml::from_str("enable: true\nrespect-rules: true\ndefault-nameserver: [1.1.1.1, 8.8.8.8]\nproxy-server-nameserver: ['https://1.1.1.1/dns-query', 'https://8.8.8.8/dns-query']\nnameserver: ['https://1.1.1.1/dns-query', 'https://8.8.8.8/dns-query']\n").map_err(|e| e.to_string())?;
    dns["ipv6"] = Value::from(tun);
    dns["enhanced-mode"] = Value::from(if tun { "fake-ip" } else { "redir-host" });
    if tun {
        dns["fake-ip-range"] = Value::from("198.19.0.1/16");
    }
    out.insert("dns".into(), dns);
    let tunnel: Value = serde_yaml::from_str(if tun {
        "enable: true\ndevice: akimov\nstack: mixed\nauto-route: true\nauto-detect-interface: true\nstrict-route: true\nmtu: 1500\ninet6-address: ['fdfe:dcba:9877::1/126']\ndns-hijack: ['any:53', 'tcp://any:53']\n"
    } else { "enable: false" }).map_err(|e| e.to_string())?;
    out.insert("tun".into(), tunnel);

    let nodes = names(sub, "proxies").len();
    let yaml = serde_yaml::to_string(&Value::Mapping(out)).map_err(|e| e.to_string())?;
    Ok(Built { yaml, main, nodes })
}

/// Rebuild untrusted client YAML before giving it to the SYSTEM core.
/// HTTP providers rely on mihomo's safe-path checks: use a private runtime home,
/// no sensitive key files, and a clean environment without path-check overrides.
pub fn privileged_config(yaml: &str, ctl: u16, secret: &str) -> Result<String, String> {
    if yaml.len() > 8 * 1024 * 1024 || ctl == 0 || secret.is_empty() {
        return Err("Invalid helper configuration.".into());
    }
    let parsed = parse(yaml)?;
    let mixed = parsed["mixed-port"]
        .as_u64()
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p > 0 && *p != ctl)
        .ok_or("Invalid helper proxy port.")?;

    fn strings(value: &Value) -> bool {
        value
            .as_sequence()
            .is_some_and(|items| items.iter().all(Value::is_string))
    }
    fn audit(value: &Value, depth: usize, proxy: bool) -> Result<(), String> {
        if depth > 64 {
            return Err("Helper configuration is nested too deeply.".into());
        }
        match value {
            Value::Mapping(map) => {
                for (key, item) in map {
                    let key = key.as_str().ok_or("Invalid helper configuration key.")?;
                    match key {
                        "script" | "script-file" | "restls-script" | "command" | "exec"
                        | "post-up" | "post-down" | "env" | "environment" | "plugin-path"
                        | "ca-file" | "cert-file" | "key-file" | "certificate-file"
                        | "private-key-file" | "state-dir" | "planet" | "override-expr" => {
                            return Err(format!("{key} is not supported by the helper."));
                        }
                        "certificate" => {
                            let text = item.as_str().ok_or("Invalid inline certificate.")?;
                            if !text.is_empty()
                                && !(text.trim_start().lines().next()
                                    == Some("-----BEGIN CERTIFICATE-----")
                                    && text.contains('\n')
                                    && text.contains("-----END CERTIFICATE-----"))
                            {
                                return Err(
                                    "Certificate files are not supported by the helper.".into()
                                );
                            }
                        }
                        "private-key" => {
                            let text = item.as_str().ok_or("Invalid inline private key.")?;
                            // WireGuard and MASQUE decode their key as data, never as a file path.
                            let inline = proxy
                                && matches!(
                                    map.get("type").and_then(Value::as_str),
                                    Some("wireguard" | "masque")
                                );
                            if !inline
                                && !text.is_empty()
                                && !(text.trim_start().starts_with("-----BEGIN ")
                                    && text.lines().any(|line| line.ends_with("PRIVATE KEY-----"))
                                    && text.contains('\n')
                                    && text.contains("-----END "))
                            {
                                return Err(
                                    "Private key files are not supported by the helper.".into()
                                );
                            }
                        }
                        "plugin"
                            if !matches!(
                                item.as_str(),
                                Some(
                                    "" | "obfs"
                                        | "v2ray-plugin"
                                        | "gost-plugin"
                                        | "shadow-tls"
                                        | "restls"
                                        | "jls"
                                        | "kcptun"
                                )
                            ) =>
                        {
                            return Err("External plugins are not supported by the helper.".into());
                        }
                        _ => {}
                    }
                    if (key.ends_with("-opts")
                        || matches!(
                            key,
                            "health-check" | "override" | "header" | "headers" | "smux"
                        ))
                        && !item.is_mapping()
                    {
                        return Err(format!("Invalid {key} options."));
                    }
                    audit(item, depth + 1, key == "proxies" || key == "payload")?;
                }
            }
            Value::Sequence(items) => {
                for item in items {
                    audit(item, depth + 1, proxy)?;
                }
            }
            Value::Tagged(_) => return Err("YAML tags are not supported by the helper.".into()),
            _ => {}
        }
        Ok(())
    }
    let mut sub = Mapping::new();
    for key in KEEP {
        let Some(value) = parsed.get(key) else {
            continue;
        };
        let valid = match key {
            "proxies" | "proxy-groups" => value.as_sequence().is_some_and(|items| {
                items.iter().all(|item| {
                    item.is_mapping() && item["name"].is_string() && item["type"].is_string()
                })
            }),
            "proxy-providers" | "rule-providers" => value.as_mapping().is_some_and(|providers| {
                providers.iter().all(|(name, provider)| {
                    name.is_string()
                        && provider.is_mapping()
                        && matches!(provider["type"].as_str(), Some("http" | "inline"))
                        && (provider["type"].as_str() != Some("http")
                            || provider["url"].as_str().is_some_and(|url| {
                                url.starts_with("https://") || url.starts_with("http://")
                            }))
                        && (provider.get("payload").is_none()
                            || if key == "proxy-providers" {
                                provider["payload"].as_sequence().is_some_and(|items| {
                                    items.iter().all(|item| {
                                        item.is_mapping()
                                            && item["name"].is_string()
                                            && item["type"].is_string()
                                    })
                                })
                            } else {
                                strings(&provider["payload"])
                            })
                })
            }),
            "rules" => strings(value),
            "sub-rules" => value.as_mapping().is_some_and(|rules| {
                rules
                    .iter()
                    .all(|(name, rules)| name.is_string() && strings(rules))
            }),
            _ => false,
        };
        if !valid {
            return Err(format!("Invalid helper {key}."));
        }
        audit(value, 0, key == "proxies")?;
        sub.insert(key.into(), value.clone());
    }
    // Bound the recursive ranking that build() performs on the untrusted group graph.
    fn group_work(
        name: &str,
        groups: &std::collections::HashMap<&str, &Value>,
        depth: usize,
        budget: &mut usize,
    ) -> Result<(), String> {
        if depth > 32 || *budget == 0 {
            return Err("Proxy group references are too complex for the helper.".into());
        }
        *budget -= 1;
        if let Some(group) = groups.get(name) {
            if let Some(items) = group.get("proxies") {
                for item in items
                    .as_sequence()
                    .ok_or("Invalid helper proxy group members.")?
                {
                    group_work(
                        item.as_str().ok_or("Invalid helper proxy group member.")?,
                        groups,
                        depth + 1,
                        budget,
                    )?;
                }
            }
        }
        Ok(())
    }
    let groups: std::collections::HashMap<_, _> = sub
        .get("proxy-groups")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .map(|group| (group["name"].as_str().unwrap(), group))
        .collect();
    let mut budget = 20_000;
    for name in groups.keys() {
        group_work(name, &groups, 0, &mut budget)?;
    }
    // Client-built profiles already have this prefix; preserve the remaining rule order.
    if let Some(rules) = sub.get_mut("rules").and_then(Value::as_sequence_mut) {
        if rules
            .iter()
            .take(SYSTEM_RULES.len())
            .filter_map(Value::as_str)
            .eq(SYSTEM_RULES)
        {
            rules.drain(..SYSTEM_RULES.len());
        }
    }
    Ok(build(&Value::Mapping(sub), &[], mixed, ctl, secret, true)?.yaml)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxy_and_latency_use_core_dns_without_changing_system_dns() {
        let sub =
            parse("proxies: [{name: Node, type: socks5, server: example.invalid, port: 9}]\n")
                .unwrap();
        let built = build(&sub, &[], 1111, 2222, "secret", false).unwrap();
        let value: Value = serde_yaml::from_str(&built.yaml).unwrap();
        assert_eq!(value["dns"]["enable"].as_bool(), Some(true));
        assert!(value["dns"]["listen"].as_str().is_none_or(str::is_empty));
        assert!(value["dns"]["proxy-server-nameserver"]
            .as_sequence()
            .is_some_and(|servers| !servers.is_empty()));
        assert_eq!(value["tun"]["enable"].as_bool(), Some(false));
    }
    #[test]
    fn offline_picker_includes_regular_servers_and_keeps_automatic_groups() {
        let sub = parse("proxies:\n- {name: Russia, type: socks5, server: 127.0.0.1, port: 9}\n- {name: Finland, type: socks5, server: 127.0.0.1, port: 9}\n- {name: Finland LTE 1, type: socks5, server: 127.0.0.1, port: 9}\n- {name: Finland LTE 2, type: socks5, server: 127.0.0.1, port: 9}\n- {name: test1, type: socks5, server: 127.0.0.1, port: 9}\nproxy-groups:\n- {name: Main, type: select, include-all: true, exclude-filter: 'LTE [0-9]', proxies: [Auto, Finland LTE auto]}\n- {name: Auto, type: url-test, include-all: true, exclude-filter: Russia, proxies: []}\n- {name: Finland LTE auto, type: url-test, include-all: true, filter: 'Finland LTE [0-9]', proxies: []}\nrules: ['MATCH,Main']").unwrap();
        let built = build(&sub, &[], 1111, 2222, "secret", false).unwrap();
        let profile = picker_profile(&built.yaml, &built.main).unwrap();
        let list: Vec<_> = profile["proxy-groups"][0]["proxies"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap())
            .collect();
        assert_eq!(
            list,
            ["Auto", "Russia", "Finland", "Finland LTE auto", "test1"]
        );
        assert_eq!(
            profile["proxy-groups"][2]["proxies"]
                .as_sequence()
                .unwrap()
                .len(),
            2
        );
        assert!(
            picker_profile("proxy-groups: [{name: Main, filter: '[invalid'}]", "Main").is_err()
        );
    }
    #[test]
    fn main_picker_follows_panel_host_order_with_groups_at_their_member_position() {
        let sub = parse("proxies:\n - {name: Russia, type: socks5, server: 127.0.0.1, port: 9}\n - {name: Finland, type: socks5, server: 127.0.0.1, port: 9}\n - {name: LTE1, type: socks5, server: 127.0.0.1, port: 9}\n - {name: LTE2, type: socks5, server: 127.0.0.1, port: 9}\nproxy-groups:\n - {name: Main, type: select, proxies: [Auto, LTE, Russia, Finland]}\n - {name: Auto, type: url-test, proxies: [Russia, Finland, LTE]}\n - {name: LTE, type: url-test, proxies: [LTE1, LTE2]}\nrules: ['MATCH,Main']\n").unwrap();
        let built = build(&sub, &[], 1111, 2222, "secret", false).unwrap();
        let v: Value = serde_yaml::from_str(&built.yaml).unwrap();
        let names: Vec<_> = v["proxy-groups"][0]["proxies"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(names, ["Auto", "Russia", "Finland", "LTE"]);
        assert_eq!(v["proxy-groups"][2]["proxies"][0].as_str(), Some("LTE1"));
    }

    const SUB: &str = "port: 7890\nexternal-controller: 0.0.0.0:9090\nproxies:\n  - {name: A, type: socks5, server: 1.1.1.1, port: 1}\nproxy-groups:\n  - {name: GO, type: select, proxies: [A]}\nrules:\n  - DOMAIN-SUFFIX,ru,DIRECT\n  - MATCH,GO\n";

    #[test]
    fn user_rules_go_first_and_base_is_ours() {
        let sub = parse(SUB).unwrap();
        let rules = vec![
            Rule {
                id: 1,
                kind: "DOMAIN-SUFFIX".into(),
                value: "example.com".into(),
                target: "@proxy".into(),
                on: true,
            },
            Rule {
                id: 2,
                kind: "IP-CIDR".into(),
                value: "8.8.8.8/32".into(),
                target: "@block".into(),
                on: true,
            },
            Rule {
                id: 3,
                kind: "DOMAIN".into(),
                value: "off.example".into(),
                target: "@direct".into(),
                on: false,
            },
        ];
        let b = build(&sub, &rules, 1111, 2222, "s3", false).unwrap();
        assert_eq!(b.main, "GO");
        let v: Value = serde_yaml::from_str(&b.yaml).unwrap();
        let r: Vec<&str> = v["rules"]
            .as_sequence()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(r[4], "DOMAIN-SUFFIX,example.com,GO");
        assert_eq!(r[5], "IP-CIDR,8.8.8.8/32,REJECT,no-resolve");
        assert_eq!(r[6], "DOMAIN-SUFFIX,ru,DIRECT");
        assert_eq!(*r.last().unwrap(), "MATCH,GO");
        assert!(!b.yaml.contains("off.example"));
        assert_eq!(v["external-controller"].as_str(), Some("127.0.0.1:2222"));
        assert!(v.get("port").is_none());
        assert_eq!(v["allow-lan"].as_bool(), Some(false));
    }

    #[test]
    fn rejects_non_profiles() {
        assert!(parse("<html>").is_err());
        assert!(parse("rules: []").is_err());
    }

    #[test]
    fn provider_only_profile_populates_selector() {
        let sub = parse("proxy-providers:\n  remote: {type: http, url: 'https://example.com/nodes', path: ./nodes.yaml}\n").unwrap();
        let b = build(&sub, &[], 1111, 2222, "secret", false).unwrap();
        let v: Value = serde_yaml::from_str(&b.yaml).unwrap();
        assert_eq!(v["proxy-groups"][0]["use"][0].as_str(), Some("remote"));
    }

    #[test]
    fn rejects_rule_injection_and_invalid_ranges() {
        let sub = parse(SUB).unwrap();
        for (kind, value, target) in [
            ("DOMAIN", "example.com,DIRECT", "@proxy"),
            ("MATCH", "x", "@proxy"),
            ("IP-CIDR", "999.1.1.1/33", "@direct"),
            ("DST-PORT", "70000", "@block"),
            ("DOMAIN", "example.com", "REJECT"),
        ] {
            let rules = [Rule {
                id: 1,
                kind: kind.into(),
                value: value.into(),
                target: target.into(),
                on: true,
            }];
            assert!(
                build(&sub, &rules, 1111, 2222, "secret", false).is_err(),
                "{kind}: {value}"
            );
        }
    }

    #[test]
    fn provider_cache_paths_cannot_escape_the_core_directory() {
        let sub = parse("proxy-providers:\n  remote: {type: http, url: 'https://example.com/nodes', path: '../../settings.json'}\n").unwrap();
        let b = build(&sub, &[], 1111, 2222, "secret", false).unwrap();
        let v: Value = serde_yaml::from_str(&b.yaml).unwrap();
        assert_eq!(
            v["proxy-providers"]["remote"]["path"].as_str(),
            Some("./providers/proxy-providers-0.yaml")
        );
    }

    #[test]
    fn automatic_match_group_remains_default_but_can_be_selected_manually() {
        let sub = parse("proxies:\n  - {name: A, type: socks5, server: 127.0.0.1, port: 9}\nproxy-groups:\n  - {name: Auto, type: url-test, proxies: [A], url: 'https://example.com', interval: 300}\nrules: ['MATCH,Auto']\n").unwrap();
        let b = build(&sub, &[], 1111, 2222, "secret", false).unwrap();
        let v: Value = serde_yaml::from_str(&b.yaml).unwrap();
        assert_eq!(v["proxy-groups"][0]["type"].as_str(), Some("select"));
        assert_eq!(v["proxy-groups"][0]["proxies"][0].as_str(), Some("Auto"));
        assert_eq!(
            v["rules"].as_sequence().unwrap().last().unwrap().as_str(),
            Some(format!("MATCH,{}", b.main).as_str())
        );
    }

    #[test]
    fn tun_owns_routes_and_dns_for_both_ip_families() {
        let b = build(&parse(SUB).unwrap(), &[], 1111, 2222, "secret", true).unwrap();
        let v: Value = serde_yaml::from_str(&b.yaml).unwrap();
        assert_eq!(v["tun"]["enable"].as_bool(), Some(true));
        assert_eq!(v["tun"]["auto-route"].as_bool(), Some(true));
        assert_eq!(v["tun"]["strict-route"].as_bool(), Some(true));
        assert_eq!(v["tun"]["device"].as_str(), Some("akimov"));
        assert_eq!(v["ipv6"].as_bool(), Some(true));
        assert_eq!(v["dns"]["enable"].as_bool(), Some(true));
        assert_eq!(v["dns"]["enhanced-mode"].as_str(), Some("fake-ip"));
    }

    #[test]
    fn privileged_profile_rebuilds_owned_settings_and_preserves_rules() {
        let built = build(&parse(SUB).unwrap(), &[], 1111, 2222, "client", false).unwrap();
        let input = format!("{}external-ui: C:/Windows\nexternal-controller-pipe: attacker\npost-up: calc.exe\nlisteners: [{{type: mixed, port: 8888}}]\n", built.yaml);
        let safe = privileged_config(&input, 3333, "private").unwrap();
        let v: Value = serde_yaml::from_str(&safe).unwrap();
        assert_eq!(v["mixed-port"].as_u64(), Some(1111));
        assert_eq!(v["external-controller"].as_str(), Some("127.0.0.1:3333"));
        assert_eq!(v["secret"].as_str(), Some("private"));
        assert_eq!(v["bind-address"].as_str(), Some("127.0.0.1"));
        assert_eq!(v["allow-lan"].as_bool(), Some(false));
        assert_eq!(v["tun"]["enable"].as_bool(), Some(true));
        assert_eq!(v["tun"]["device"].as_str(), Some("akimov"));
        assert_eq!(v["profile"]["store-selected"].as_bool(), Some(true));
        assert_eq!(
            v["rules"],
            serde_yaml::from_str::<Value>(&built.yaml).unwrap()["rules"]
        );
        for key in [
            "external-ui",
            "external-controller-pipe",
            "post-up",
            "listeners",
        ] {
            assert!(v.get(key).is_none(), "{key}");
        }
    }

    #[test]
    fn privileged_profile_contains_provider_cache_paths() {
        let input = "mixed-port: 1111\nproxy-providers:\n  remote: {type: http, url: 'https://example.com/nodes', path: 'C:/Windows/evil.exe'}\nrule-providers:\n  rules: {type: http, behavior: domain, url: 'https://example.com/rules', path: '../../evil.exe'}\n";
        let safe = privileged_config(input, 2222, "private").unwrap();
        let v: Value = serde_yaml::from_str(&safe).unwrap();
        assert_eq!(
            v["proxy-providers"]["remote"]["path"].as_str(),
            Some("./providers/proxy-providers-0.yaml")
        );
        assert_eq!(
            v["rule-providers"]["rules"]["path"].as_str(),
            Some("./providers/rule-providers-0.yaml")
        );
        assert!(
            privileged_config(&input.replace("type: http", "type: file"), 2222, "private").is_err()
        );
    }

    #[test]
    fn privileged_profile_rejects_nested_file_and_execution_options() {
        for extra in [
            "certificate: 'C:/Windows/cert.pem'",
            "private-key: './secret.pem'",
            "ws-opts: {certificate: './secret.pem'}",
            "xhttp-opts: {download-settings: {private-key: './secret.pem'}}",
            "plugin: 'C:/Windows/evil.exe'",
            "plugin-opts: {certificate: './secret.pem'}",
            "script: evil.js",
            "post-up: calc.exe",
            "state-dir: 'C:/Windows'",
            "planet: './secret'",
            "ws-opts: wrong",
            "smux: wrong",
        ] {
            let input = format!("mixed-port: 1111\nproxies: [{{name: A, type: vless, server: example.com, port: 443, {extra}}}]");
            assert!(
                privileged_config(&input, 2222, "private").is_err(),
                "{extra}"
            );
        }
        let inline = "mixed-port: 1111\nproxy-providers: {inline: {type: inline, payload: [{name: A, type: ssh, private-key: './config.yaml'}]}}";
        assert!(privileged_config(inline, 2222, "private").is_err());
    }

    #[test]
    fn privileged_profile_accepts_reality_and_inline_wireguard_keys() {
        let input = "mixed-port: 1111\nproxies:\n- {name: A, type: vless, server: example.com, port: 443, uuid: 12345678-1234-1234-1234-123456789012, tls: true, flow: xtls-rprx-vision, servername: example.com, client-fingerprint: chrome, reality-opts: {public-key: abcdefghijklmnopqrstuvwxyz, short-id: abcd}, ws-opts: {path: /socket}}\n- {name: B, type: wireguard, private-key: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=, public-key: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=, server: example.com, port: 51820}\n";
        let safe = privileged_config(input, 2222, "private").unwrap();
        let v: Value = serde_yaml::from_str(&safe).unwrap();
        assert_eq!(
            v["proxies"][0]["reality-opts"]["short-id"].as_str(),
            Some("abcd")
        );
        assert_eq!(v["proxies"][0]["ws-opts"]["path"].as_str(), Some("/socket"));
        assert_eq!(
            v["proxies"][1]["private-key"].as_str(),
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")
        );
    }

    #[test]
    fn privileged_profile_rejects_bad_ports_shapes_and_large_input() {
        for port in ["0", "2222", "65536", "-1", "'1111'", "null"] {
            assert!(
                privileged_config(&format!("{SUB}mixed-port: {port}\n"), 2222, "private").is_err(),
                "{port}"
            );
        }
        for extra in [
            "proxy-groups: wrong",
            "proxy-providers: []",
            "rule-providers: {bad: []}",
            "sub-rules: []",
            "rules: [3]",
        ] {
            // SUB has groups and rules already: use a single plain node to avoid duplicate YAML keys.
            let input = format!("mixed-port: 1111\nproxies: [{{name: A, type: socks5}}]\n{extra}");
            assert!(
                privileged_config(&input, 2222, "private").is_err(),
                "{extra}"
            );
        }
        assert!(privileged_config(&" ".repeat(8 * 1024 * 1024 + 1), 2222, "private").is_err());
    }

    #[test]
    fn privileged_profile_bounds_group_reference_work() {
        for (count, duplicate) in [(40, false), (18, true)] {
            let mut input =
                "mixed-port: 1111\nproxies: [{name: A, type: socks5}]\nproxy-groups:\n".to_string();
            for i in 0..count {
                let next = if i + 1 == count {
                    "A".into()
                } else {
                    format!("G{}", i + 1)
                };
                let refs = if duplicate {
                    format!("{next}, {next}")
                } else {
                    next
                };
                input.push_str(&format!(
                    "- {{name: G{i}, type: select, proxies: [{refs}]}}\n"
                ));
            }
            let error = privileged_config(&input, 2222, "private").unwrap_err();
            assert!(error.contains("references"), "{error}");
        }
    }
}
