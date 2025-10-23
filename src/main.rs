use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;
use dns_lookup::lookup_host;
use regex::Regex;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "muko")]
#[command(about = "A command-line utility to manage host file entries", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a domain to the hosts file
    Add {
        /// Domain name to add
        domain_name: String,

        /// IP address (defaults to 127.0.0.1)
        #[arg(long, default_value = "127.0.0.1")]
        ip: String,

        /// Alias for the domain (defaults to domain_name if not provided)
        #[arg(long)]
        alias: Option<String>,
    },
    /// Set a domain to DEV mode (uncomment to use custom IP)
    Dev {
        /// Domain name or alias
        identifier: String,
    },
    /// Set a domain to PROD mode (comment out to use real IP)
    Prod {
        /// Domain name or alias
        identifier: String,
    },
}

#[derive(Debug)]
struct MukoManagedDomain {
    ip: String,
    domain: String,
    alias: Option<String>,
    active: bool, // whether this line is commented out or not
    prod_ip: Option<String>, // Real IP from DNS resolution
}

const HOSTS_FILE: &str = "/etc/hosts";
const MUKO_TAG: &str = "#muko:";

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Add { domain_name, ip, alias }) => {
            // Use domain_name as alias if not provided
            let alias_value = alias.unwrap_or_else(|| domain_name.clone());
            if let Err(e) = add_domain(&domain_name, &ip, &alias_value) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Dev { identifier }) => {
            if let Err(e) = set_mode(&identifier, true) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Prod { identifier }) => {
            if let Err(e) = set_mode(&identifier, false) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            // No command provided, just print the table
            match parse_muko_entries() {
                Ok(entries) => {
                    println!("Muko-managed domains:");
                    print_muko_table(&entries);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn add_domain(domain: &str, ip: &str, alias: &str) -> io::Result<()> {
    // Read the current hosts file
    let path = Path::new(HOSTS_FILE);
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut lines = Vec::new();
    let mut domain_found = false;
    let new_entry = format!("{} {} {} {}", ip, domain, MUKO_TAG, alias);

    // Process existing lines
    for line in reader.lines() {
        let line = line?;

        // Quick check: if line doesn't contain the domain, keep it
        if !line.contains(domain) {
            lines.push(line);
            continue;
        }

        // Line contains domain - need to parse it carefully
        let trimmed = line.trim_start();

        // Handle commented lines (could be "# 127.0.0.1 draftlab.app")
        let content = if trimmed.starts_with('#') {
            trimmed.trim_start_matches('#').trim_start()
        } else {
            trimmed
        };

        // Split by whitespace to get [IP, hostname1, hostname2, ...]
        // Stop at # if there's an inline comment
        let before_comment = content.split('#').next().unwrap_or("");
        let tokens: Vec<&str> = before_comment.split_whitespace().collect();

        // Check if this is a valid host entry: at least IP + hostname
        if tokens.len() >= 2 {
            // tokens[0] should be an IP, tokens[1..] are hostnames
            // Check if any hostname exactly matches our domain
            if tokens[1..].iter().any(|&h| h == domain) {
                // Found duplicate - skip this line, we'll replace it
                domain_found = true;
                continue;
            }
        }

        // Not a match, keep the line
        lines.push(line);
    }

    // Add the new entry
    lines.push(new_entry);

    // Write back to the hosts file
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)?;

    for line in &lines {
        writeln!(file, "{}", line)?;
    }

    // Notify the user
    if domain_found {
        println!("✓ Domain '{}' already existed and has been overwritten", domain);
    } else {
        println!("✓ Domain '{}' has been added to {}", domain, HOSTS_FILE);
    }
    println!("  {} {} {} {}", ip, domain, MUKO_TAG, alias);

    // Print the updated muko-managed domains table
    println!("\nMuko-managed domains:");
    let entries = parse_muko_entries()?;
    print_muko_table(&entries);

    Ok(())
}

/// Set a muko-managed domain to DEV or PROD mode
/// dev_mode: true for DEV (uncomment), false for PROD (comment out)
fn set_mode(identifier: &str, dev_mode: bool) -> io::Result<()> {
    let path = Path::new(HOSTS_FILE);
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut lines = Vec::new();
    let mut found = false;
    let re = Regex::new(
        r"^(#)?\s*((?:\d+\.\d+\.\d+\.\d+)|(?:[0-9a-fA-F:]+))\s+(\S+)\s+#muko:\s*(\S*)"
    ).unwrap();

    for line in reader.lines() {
        let line = line?;

        // Check if this is a muko-managed line
        if line.contains(MUKO_TAG) {
            if let Some(caps) = re.captures(&line) {
                let domain = caps.get(3).map(|m| m.as_str()).unwrap();
                let alias_str = caps.get(4).map(|m| m.as_str()).unwrap();

                // Check if this line matches the identifier (domain or alias)
                if domain == identifier || alias_str == identifier {
                    found = true;
                    let is_commented = caps.get(1).is_some();

                    if dev_mode {
                        // DEV mode: uncomment if necessary
                        if is_commented {
                            // Remove the leading #
                            let uncommented = line.trim_start_matches('#').trim_start().to_string();
                            lines.push(uncommented);
                        } else {
                            // Already uncommented
                            lines.push(line);
                        }
                    } else {
                        // PROD mode: comment out if necessary
                        if !is_commented {
                            // Add # at the beginning
                            lines.push(format!("#{}", line));
                        } else {
                            // Already commented
                            lines.push(line);
                        }
                    }
                    continue;
                }
            }
        }

        // Not a match, keep the line as is
        lines.push(line);
    }

    if !found {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No muko-managed entry found for '{}'", identifier),
        ));
    }

    // Write back to the hosts file
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)?;

    for line in &lines {
        writeln!(file, "{}", line)?;
    }

    // Notify the user
    let mode_name = if dev_mode { "DEV" } else { "PROD" };
    println!("✓ Set '{}' to {} mode", identifier, mode_name);

    // Print the updated muko-managed domains table
    println!("\nMuko-managed domains:");
    let entries = parse_muko_entries()?;
    print_muko_table(&entries);

    Ok(())
}

/// Parse muko-managed entries from the hosts file
fn parse_muko_entries() -> io::Result<Vec<MukoManagedDomain>> {
    let path = Path::new(HOSTS_FILE);
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut entries = Vec::new();

    // Regex to parse muko-managed lines
    // Captures: (optional #) (IP - IPv4 or IPv6) (domain) #muko: (alias)
    // IPv4: \d+\.\d+\.\d+\.\d+
    // IPv6: [0-9a-fA-F:]+
    let re = Regex::new(
        r"^(#)?\s*((?:\d+\.\d+\.\d+\.\d+)|(?:[0-9a-fA-F:]+))\s+(\S+)\s+#muko:\s*(\S*)"
    ).unwrap();

    for line in reader.lines() {
        let line = line?;

        // Quick check: if line doesn't contain muko tag, skip it
        if !line.contains(MUKO_TAG) {
            continue;
        }

        // Try to parse with regex
        if let Some(caps) = re.captures(&line) {
            let active = caps.get(1).is_none(); // If no # at start, it's active
            let ip = caps.get(2).map(|m| m.as_str().to_string()).unwrap();
            let domain = caps.get(3).map(|m| m.as_str().to_string()).unwrap();
            let alias_str = caps.get(4).map(|m| m.as_str().to_string()).unwrap();

            let alias = if alias_str.is_empty() {
                None
            } else {
                Some(alias_str)
            };

            // Resolve DNS to get the real IP address
            // Only do this in PROD mode; in DEV mode we don't need it
            let prod_ip = if !active {
                // PROD mode: retry up to 3 times until we get a different IP than dev_ip
                let mut resolved_ip = None;
                for attempt in 1..=3 {
                    if let Some(lookup_ip) = lookup_host(&domain)
                        .ok()
                        .and_then(|ips| ips.into_iter().next())
                        .map(|ip| ip.to_string())
                    {
                        // If the resolved IP is different from dev IP, we found the real prod IP
                        if lookup_ip != ip {
                            resolved_ip = Some(lookup_ip);
                            break;
                        }
                        resolved_ip = Some(lookup_ip);
                    }

                    // Wait a bit before retrying (except on last attempt)
                    if attempt < 3 {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                resolved_ip
            } else {
                // DEV mode: no lookup needed, won't be displayed
                None
            };

            entries.push(MukoManagedDomain {
                ip,
                domain,
                alias,
                active,
                prod_ip,
            });
        }
    }

    Ok(entries)
}

/// Print muko-managed domains as a formatted table
fn print_muko_table(entries: &[MukoManagedDomain]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Mode").add_attribute(Attribute::Bold),
            Cell::new("Domain").add_attribute(Attribute::Bold),
            Cell::new("Alias").add_attribute(Attribute::Bold),
            Cell::new("Dev IP").add_attribute(Attribute::Bold),
            Cell::new("Prod IP").add_attribute(Attribute::Bold),
        ]);

    for entry in entries {
        let mode = if entry.active {
            // Not commented out = using custom IP = DEV mode
            Cell::new("DEV").fg(Color::Green)
        } else {
            // Commented out = using real IP = PROD mode
            Cell::new("PROD").fg(Color::Blue)
        };

        // Only show alias if it exists and is different from domain
        let alias_display = match &entry.alias {
            Some(alias) if alias != &entry.domain => alias.as_str(),
            _ => "",
        };

        // Only show prod IP if in PROD mode (not active)
        let prod_ip_display = if entry.active {
            ""
        } else {
            entry.prod_ip.as_deref().unwrap_or("-")
        };

        table.add_row(vec![
            mode,
            Cell::new(&entry.domain),
            Cell::new(alias_display),
            Cell::new(&entry.ip),
            Cell::new(prod_ip_display),
        ]);
    }

    println!("{}", table);
}
