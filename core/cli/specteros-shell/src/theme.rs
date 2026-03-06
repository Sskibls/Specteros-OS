// PhantomKernel Shell - Theme System
// Provides visual theming for the terminal interface

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Default,
    Fsociety,
    Allsafe,
    DarkArmy,
}

impl Theme {
    #[allow(clippy::useless_format)]
    pub fn welcome_banner(&self) -> String {
        match self {
            Theme::Default => {
                format!(r#"
╔══════════════════════════════════════════╗
║       PhantomKernel OS Shell v0.1.0        ║
║     Privacy-First Control Interface      ║
╚══════════════════════════════════════════╝
"#)
            }
            Theme::Fsociety => {
                format!(r#"
   ██████╗ ███████╗████████╗██████╗  ██████╗ 
  ██╔════╝ ██╔════╝╚══██╔══╝██╔══██╗██╔═══██╗
  ██║  ███╗█████╗     ██║   ██████╔╝██║   ██║
  ██║   ██║██╔══╝     ██║   ██╔══██╗██║   ██║
  ╚██████╔╝███████╗   ██║   ██║  ██║╚██████╔╝
   ╚═════╝ ╚══════╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ 
                                            
  [ CONTROL TERMINAL v0.1.0 ]
  [ fsociety theme active ]
  [ all your privacy are belong to us ]
"#)
            }
            Theme::Allsafe => {
                format!(r#"
┌──────────────────────────────────────────┐
│  PhantomKernel OS | Allsafe Security      │
│  Secure Control Interface v0.1.0         │
└──────────────────────────────────────────┘
"#)
            }
            Theme::DarkArmy => {
                format!(r#"
▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓
  PHANTOMKERNEL // DARKARMY MODE
  SURVEILLANCE COUNTERMEASURES ACTIVE
▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓
"#)
            }
        }
    }

    pub fn prompt_help(&self) -> String {
        match self {
            Theme::Default => "Type 'help' for commands.".to_string(),
            Theme::Fsociety => "[?] type 'help' for command list".to_string(),
            Theme::Allsafe => "Hint: Type 'help' to see available commands".to_string(),
            Theme::DarkArmy => "[INPUT] 'help' :: display commands".to_string(),
        }
    }

    pub fn prompt(&self, current_shard: &Option<String>) -> String {
        let shard = current_shard.as_deref().unwrap_or("system");
        match self {
            Theme::Default => format!("gksh [{}]$ ", shard),
            Theme::Fsociety => format!("\x1b[31mroot@fsociety\x1b[0m:\x1b[34m{}\x1b[0m$ ", shard),
            Theme::Allsafe => format!("allsafe@{}> ", shard),
            Theme::DarkArmy => format!("[DARKARMY::{}]# ", shard),
        }
    }

    pub fn style_output(&self, text: String) -> String {
        match self {
            Theme::Default => text,
            Theme::Fsociety => format!("\x1b[32m{}\x1b[0m", text), // Green
            Theme::Allsafe => format!("\x1b[36m{}\x1b[0m", text),  // Cyan
            Theme::DarkArmy => format!("\x1b[1;37m{}\x1b[0m", text), // Bold white
        }
    }

    pub fn style_error(&self, text: String) -> String {
        match self {
            Theme::Default => format!("\x1b[31m{}\x1b[0m", text),
            Theme::Fsociety => format!("\x1b[1;31m[!]\x1b[0m \x1b[31m{}\x1b[0m", text),
            Theme::Allsafe => format!("\x1b[31mError: {}\x1b[0m", text),
            Theme::DarkArmy => format!("\x1b[1;31m[ERR]\x1b[0m \x1b[31m{}\x1b[0m", text),
        }
    }

    pub fn style_section(&self, text: &str) -> String {
        match self {
            Theme::Default => format!("\x1b[1m{}\x1b[0m", text),
            Theme::Fsociety => format!("\x1b[1;35m## {}\x1b[0m", text),
            Theme::Allsafe => format!("\x1b[1;34m── {} ──\x1b[0m", text),
            Theme::DarkArmy => format!("\x1b[1;33m[{}]\x1b[0m", text),
        }
    }

    pub fn style_success(&self, text: String) -> String {
        match self {
            Theme::Default => format!("\x1b[32m{}\x1b[0m", text),
            Theme::Fsociety => format!("\x1b[1;32m[+]\x1b[0m \x1b[32m{}\x1b[0m", text),
            Theme::Allsafe => format!("\x1b[32m✓ {}\x1b[0m", text),
            Theme::DarkArmy => format!("\x1b[1;32m[OK]\x1b[0m \x1b[32m{}\x1b[0m", text),
        }
    }

    pub fn style_warning(&self, text: String) -> String {
        match self {
            Theme::Default => format!("\x1b[33m{}\x1b[0m", text),
            Theme::Fsociety => format!("\x1b[1;33m[!]\x1b[0m \x1b[33m{}\x1b[0m", text),
            Theme::Allsafe => format!("\x1b[33m⚠ {}\x1b[0m", text),
            Theme::DarkArmy => format!("\x1b[1;33m[WARN]\x1b[0m \x1b[33m{}\x1b[0m", text),
        }
    }

    pub fn style_info(&self, text: String) -> String {
        match self {
            Theme::Default => format!("\x1b[36m{}\x1b[0m", text),
            Theme::Fsociety => format!("\x1b[36m[*]\x1b[0m \x1b[36m{}\x1b[0m", text),
            Theme::Allsafe => format!("\x1b[34mℹ {}\x1b[0m", text),
            Theme::DarkArmy => format!("\x1b[1;34m[INFO]\x1b[0m \x1b[34m{}\x1b[0m", text),
        }
    }
}

impl std::str::FromStr for Theme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Theme::Default),
            "fsociety" => Ok(Theme::Fsociety),
            "allsafe" => Ok(Theme::Allsafe),
            "darkarmy" => Ok(Theme::DarkArmy),
            _ => Err(format!("Unknown theme: {}. Available: default, fsociety, allsafe, darkarmy", s)),
        }
    }
}
