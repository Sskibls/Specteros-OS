// PhantomKernel Shell - Terminal Control Interface
// Provides interactive CLI for managing PhantomKernel OS services

use anyhow::Result;
use std::io::{self, BufRead, Write};

mod commands;
mod theme;

use commands::{execute_command, CommandContext};
use theme::Theme;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    // Check for theme argument
    let theme = args.iter()
        .find(|arg| arg.starts_with("--theme="))
        .map(|arg| arg.trim_start_matches("--theme="))
        .unwrap_or("default")
        .parse::<Theme>()
        .unwrap_or(Theme::Default);
    
    // Check for non-interactive mode
    let non_interactive = args.contains(&"--cmd".to_string()) || args.contains(&"-c".to_string());
    
    if non_interactive {
        // Find command after --cmd or -c
        if let Some(pos) = args.iter().position(|arg| arg == "--cmd" || arg == "-c") {
            if let Some(cmd) = args.get(pos + 1) {
                return execute_single_command(cmd, &theme);
            }
        }
    }
    
    // Interactive mode
    println!("{}", theme.welcome_banner());
    println!("{}", theme.prompt_help());
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut context = CommandContext::new()?;
    
    loop {
        print!("{}", theme.prompt(&context.current_shard));
        stdout.flush()?;
        
        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        
        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        
        match input {
            "exit" | "quit" | "q" => {
                println!("{}", theme.style_output("Goodbye.".to_string()));
                break;
            }
            "help" | "h" | "?" => {
                print_help(&theme);
            }
            "clear" | "cls" => {
                print!("\x1B[2J\x1B[1;1H");
                stdout.flush()?;
            }
            "theme ls" => {
                list_themes(&theme);
            }
            _ => {
                match execute_command(input, &mut context, &theme) {
                    Ok(output) => {
                        if !output.is_empty() {
                            println!("{}", theme.style_output(output));
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", theme.style_error(format!("Error: {e}")));
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn execute_single_command(cmd: &str, theme: &Theme) -> Result<()> {
    let mut context = CommandContext::new()?;
    match execute_command(cmd, &mut context, theme) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{}", theme.style_output(output));
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("{}", theme.style_error(format!("Error: {e}")));
            std::process::exit(1);
        }
    }
}

fn print_help(theme: &Theme) {
    println!();
    println!("{}:", theme.style_section("Shard Management"));
    println!("  shard ls              List all persona shards");
    println!("  shard create <name>   Create a new shard");
    println!("  shard start <name>    Start a shard");
    println!("  shard stop <name>     Stop a shard");
    println!("  shard status <name>   Show shard status");
    println!();
    println!("{}:", theme.style_section("Policy Control"));
    println!("  policy ls             List policy rules");
    println!("  policy grant <shard> <resource> [duration]");
    println!("  policy revoke <shard> <resource>");
    println!("  policy check <shard> <resource>");
    println!();
    println!("{}:", theme.style_section("Network Control"));
    println!("  net status            Show network status");
    println!("  net profile <shard>   Show shard network profile");
    println!("  net kill              Enable network kill switch");
    println!("  net restore           Disable kill switch");
    println!();
    println!("{}:", theme.style_section("Airlock Transfer"));
    println!("  airlock send <from> <to> <file>");
    println!("  airlock status        Show airlock session status");
    println!();
    println!("{}:", theme.style_section("Emergency Modes"));
    println!("  panic                 Activate panic mode (kill network, lock shards)");
    println!("  mask <workspace>      Switch to decoy workspace");
    println!("  travel on|off         Toggle travel mode");
    println!();
    println!("{}:", theme.style_section("Audit & System"));
    println!("  audit ls [limit]      List recent audit events");
    println!("  audit verify          Verify audit chain integrity");
    println!("  status                Show system status");
    println!("  theme ls              List available themes");
    println!("  help                  Show this help");
    println!("  clear                 Clear screen");
    println!("  exit                  Exit shell");
    println!();
}

fn list_themes(_theme: &Theme) {
    println!("Available themes:");
    println!("  default     - Standard terminal theme");
    println!("  fsociety    - Hacker aesthetic (terminal-centric)");
    println!("  allsafe     - Clean professional look");
    println!("  darkarmy    - High-contrast strict mode");
}
