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
}
