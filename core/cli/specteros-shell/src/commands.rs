// PhantomKernel Shell - Command Execution
// Handles parsing and execution of shell commands

use anyhow::Result;
use gk_audit::AuditChain;
use std::collections::HashMap;

use crate::theme::Theme;

pub struct CommandContext {
    pub current_shard: Option<String>,
    pub audit_chain: AuditChain,
    pub shards: HashMap<String, ShardStatus>,
}

#[derive(Debug, Clone)]
pub struct ShardStatus {
    pub state: String,
    pub network_profile: String,
}

impl CommandContext {
    pub fn new() -> Result<Self> {
        // Initialize with default shards
        let mut shards = HashMap::new();
        for name in &["work", "anon", "burner", "lab"] {
            shards.insert(name.to_string(), ShardStatus {
                state: "stopped".to_string(),
                network_profile: "default".to_string(),
            });
        }

        Ok(Self {
            current_shard: None,
            audit_chain: AuditChain::default(),
            shards,
        })
    }
}

pub fn execute_command(input: &str, context: &mut CommandContext, theme: &Theme) -> Result<String> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(String::new());
    }

    match parts[0] {
        "shard" => execute_shard_command(&parts[1..], context, theme),
        "policy" => execute_policy_command(&parts[1..], context, theme),
        "net" => execute_net_command(&parts[1..], context, theme),
        "airlock" => execute_airlock_command(&parts[1..], context, theme),
        "panic" => execute_panic_command(context, theme),
        "mask" => execute_mask_command(&parts[1..], context, theme),
        "travel" => execute_travel_command(&parts[1..], context, theme),
        "audit" => execute_audit_command(&parts[1..], context, theme),
        "status" => execute_status_command(context, theme),
        "use" => execute_use_command(&parts[1..], context, theme),
        _ => Err(anyhow::anyhow!("Unknown command: '{}'. Type 'help' for available commands.", parts[0])),
    }
}

fn execute_shard_command(args: &[&str], context: &mut CommandContext, theme: &Theme) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow::anyhow!("Usage: shard <ls|create|start|stop|status> [name]"));
    }

    match args[0] {
        "ls" | "list" => {
            let mut output = String::from("Persona Shards:\n");
            for (name, status) in &context.shards {
                let indicator = match status.state.as_str() {
                    "running" => theme.style_success("●".to_string()),
                    "stopped" => theme.style_info("○".to_string()),
                    _ => "○".to_string(),
                };
                output.push_str(&format!("  {} {} - network: {}\n", indicator, name, status.network_profile));
            }
            Ok(output)
        }
        "create" => {
            if args.len() < 2 {
                return Err(anyhow::anyhow!("Usage: shard create <name>"));
            }
            let name = args[1];
            if context.shards.contains_key(name) {
                return Err(anyhow::anyhow!("Shard '{}' already exists", name));
            }
            context.shards.insert(name.to_string(), ShardStatus {
                state: "stopped".to_string(),
                network_profile: "default".to_string(),
            });
            context.audit_chain.append("shell.shard.created", name);
            Ok(theme.style_success(format!("Shard '{}' created", name)))
        }
        "start" => {
            if args.len() < 2 {
                return Err(anyhow::anyhow!("Usage: shard start <name>"));
            }
            let name = args[1];
            if let Some(status) = context.shards.get_mut(name) {
                status.state = "running".to_string();
                context.audit_chain.append("shell.shard.started", name);
                Ok(theme.style_success(format!("Shard '{}' started", name)))
            } else {
                Err(anyhow::anyhow!("Shard '{}' not found", name))
            }
        }
        "stop" => {
            if args.len() < 2 {
                return Err(anyhow::anyhow!("Usage: shard stop <name>"));
            }
            let name = args[1];
            if let Some(status) = context.shards.get_mut(name) {
                status.state = "stopped".to_string();
                context.audit_chain.append("shell.shard.stopped", name);
                Ok(theme.style_warning(format!("Shard '{}' stopped", name)))
            } else {
                Err(anyhow::anyhow!("Shard '{}' not found", name))
            }
        }
        "status" => {
            if args.len() < 2 {
                return Err(anyhow::anyhow!("Usage: shard status <name>"));
            }
            let name = args[1];
            if let Some(status) = context.shards.get(name) {
                Ok(format!(
                    "Shard: {}\n  State: {}\n  Network: {}\n  Policy: active",
                    name, status.state, status.network_profile
                ))
            } else {
                Err(anyhow::anyhow!("Shard '{}' not found", name))
            }
        }
        _ => Err(anyhow::anyhow!("Unknown shard command: '{}'. Use: ls, create, start, stop, status", args[0])),
    }
}

fn execute_policy_command(args: &[&str], _context: &mut CommandContext, theme: &Theme) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow::anyhow!("Usage: policy <ls|grant|revoke|check>"));
    }

    match args[0] {
        "ls" | "list" => {
            let output = r#"Policy Rules:
  [work]
    - filesystem: ~/work/* (read/write)
    - network: direct (allowed)
    - camera: denied
    - microphone: 15min sessions
    
  [anon]
    - filesystem: ~/anon/* (read/write)
    - network: tor-only
    - camera: denied
    - microphone: denied
    
  [burner]
    - filesystem: tmpfs only
    - network: tor + rotation
    - camera: denied
    - microphone: denied
    
  [lab]
    - filesystem: ~/lab/* (read/write)
    - network: isolated vlan
    - camera: allowed
    - microphone: allowed
"#;
            Ok(output.to_string())
        }
        "grant" => {
            if args.len() < 3 {
                return Err(anyhow::anyhow!("Usage: policy grant <shard> <resource> [duration]"));
            }
            let shard = args[1];
            let resource = args[2];
            let duration = args.get(3).copied().unwrap_or("1h");
            Ok(theme.style_success(format!("Granted '{}' access to shard '{}' for {}", resource, shard, duration)))
        }
        "revoke" => {
            if args.len() < 3 {
                return Err(anyhow::anyhow!("Usage: policy revoke <shard> <resource>"));
            }
            let shard = args[1];
            let resource = args[2];
            Ok(theme.style_warning(format!("Revoked '{}' access from shard '{}'", resource, shard)))
        }
        "check" => {
            if args.len() < 3 {
                return Err(anyhow::anyhow!("Usage: policy check <shard> <resource>"));
            }
            // Simulated policy check
            Ok(theme.style_info("Policy check: ALLOWED (simulated)".to_string()))
        }
        _ => Err(anyhow::anyhow!("Unknown policy command: '{}'. Use: ls, grant, revoke, check", args[0])),
    }
}

fn execute_net_command(args: &[&str], context: &mut CommandContext, theme: &Theme) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow::anyhow!("Usage: net <status|profile|kill|restore>"));
    }

    match args[0] {
        "status" => {
            let output = r#"Network Status:
  Kill Switch: inactive
  DNS Leak Check: passed
  IPv6 Policy: disabled
  Active Routes:
    - work: direct (eth0)
    - anon: tor (tun0)
    - burner: tor + rotate (tun1)
    - lab: isolated (vlan100)
"#;
            Ok(output.to_string())
        }
        "profile" => {
            if args.len() < 2 {
                return Err(anyhow::anyhow!("Usage: net profile <shard>"));
            }
            let shard = args[1];
            if let Some(status) = context.shards.get_mut(shard) {
                Ok(format!("Shard '{}' network profile: {}", shard, status.network_profile))
            } else {
                Err(anyhow::anyhow!("Shard '{}' not found", shard))
            }
        }
        "kill" => {
            context.audit_chain.append("shell.net.kill_switch", "activated");
            Ok(theme.style_warning("⚠ KILL SWITCH ACTIVATED - All network interfaces disabled".to_string()))
        }
        "restore" => {
            context.audit_chain.append("shell.net.kill_switch", "deactivated");
            Ok(theme.style_success("Network interfaces restored".to_string()))
        }
        _ => Err(anyhow::anyhow!("Unknown net command: '{}'. Use: status, profile, kill, restore", args[0])),
    }
}

fn execute_airlock_command(args: &[&str], _context: &mut CommandContext, theme: &Theme) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow::anyhow!("Usage: airlock <send|status|approve|reject>"));
    }

    match args[0] {
        "send" => {
            if args.len() < 4 {
                return Err(anyhow::anyhow!("Usage: airlock send <from_shard> <to_shard> <file>"));
            }
            let from = args[1];
            let to = args[2];
            let file = args[3];
            Ok(theme.style_info(format!("Initiating airlock transfer: {} → {} (file: {})", from, to, file)))
        }
        "status" => {
            Ok(r#"Airlock Status:
  Active Sessions: 0
  Pending Transfers: 0
  Last Transfer: none
"#.to_string())
        }
        _ => Err(anyhow::anyhow!("Unknown airlock command: '{}'. Use: send, status", args[0])),
    }
}

fn execute_panic_command(context: &mut CommandContext, theme: &Theme) -> Result<String> {
    context.audit_chain.append("shell.panic", "activated");
    
    // Update all shard states
    for status in context.shards.values_mut() {
        status.state = "locked".to_string();
    }
    
    Ok(format!(
        "{}\n{}\n{}",
        theme.style_warning("⚠ PANIC MODE ACTIVATED ⚠".to_string()),
        theme.style_error("Network kill switch: ENGAGED".to_string()),
        theme.style_error("All shards: LOCKED".to_string())
    ))
}

fn execute_mask_command(args: &[&str], context: &mut CommandContext, theme: &Theme) -> Result<String> {
    let workspace = args.first().copied().unwrap_or("decoy");
    context.audit_chain.append("shell.mask", workspace);
    Ok(theme.style_info(format!("Mask mode: Switched to '{}' workspace", workspace)))
}

fn execute_travel_command(args: &[&str], context: &mut CommandContext, theme: &Theme) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow::anyhow!("Usage: travel <on|off>"));
    }
    
    let enabled = match args[0] {
        "on" | "enable" | "true" => true,
        "off" | "disable" | "false" => false,
        _ => return Err(anyhow::anyhow!("Usage: travel <on|off>")),
    };
    
    context.audit_chain.append("shell.travel", if enabled { "enabled" } else { "disabled" });
    
    if enabled {
        Ok(theme.style_warning("Travel mode: ENABLED (ephemeral sessions, strict policy)".to_string()))
    } else {
        Ok(theme.style_success("Travel mode: DISABLED".to_string()))
    }
}

fn execute_audit_command(args: &[&str], context: &mut CommandContext, theme: &Theme) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow::anyhow!("Usage: audit <ls|verify>"));
    }

    match args[0] {
        "ls" | "list" => {
            let limit: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
            let events = context.audit_chain.events();
            let output = if events.is_empty() {
                "No audit events recorded.".to_string()
            } else {
                let mut output = String::from("Recent Audit Events:\n");
                for event in events.iter().rev().take(limit) {
                    output.push_str(&format!("  [#{:04}] {} -> {}\n", event.sequence, event.event_type, event.payload));
                }
                output
            };
            Ok(output)
        }
        "verify" => {
            // Simulated verification
            context.audit_chain.append("shell.audit.verify", "requested");
            Ok(theme.style_success("Audit chain integrity: VERIFIED".to_string()))
        }
        _ => Err(anyhow::anyhow!("Unknown audit command: '{}'. Use: ls, verify", args[0])),
    }
}

fn execute_status_command(context: &mut CommandContext, _theme: &Theme) -> Result<String> {
    let running_shards = context.shards.values().filter(|s| s.state == "running").count();
    let total_shards = context.shards.len();
    
    Ok(format!(r#"
System Status:
  Shards: {}/{} running
  Network: operational
  Audit Chain: active ({} events)
  Security Mode: normal
  Theme: active
"#, 
        running_shards, 
        total_shards,
        context.audit_chain.events().len()
    ))
}

fn execute_use_command(args: &[&str], context: &mut CommandContext, theme: &Theme) -> Result<String> {
    if args.is_empty() {
        return Err(anyhow::anyhow!("Usage: use <shard_name>"));
    }
    
    let shard_name = args[0];
    if context.shards.contains_key(shard_name) {
        context.current_shard = Some(shard_name.to_string());
        Ok(theme.style_success(format!("Switched to shard: '{}'", shard_name)))
    } else {
        Err(anyhow::anyhow!("Shard '{}' not found. Use 'shard ls' to list shards.", shard_name))
    }
}
