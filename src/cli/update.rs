use std::path::Path;
use std::fs::File;
use anyhow::{anyhow, Result};
use std::io;
use clap::Parser;
use std::io::Write;
use crate::env_file::{EnvFile, EnvFileRow};

/// Read settings from INPUT and update OUTPUT
#[derive(Parser, Debug)]
pub struct Cmd {
    /// Name of the file to load as input (default: .env.example)
    #[arg(default_value = ".env.example")]
    input: String,

    /// Name of the file to load as output (default: .env)
    #[arg(default_value = ".env")]
    output: String,

    /// Skip already filled variables
    #[arg(short = 'e', long, default_value_t = false)]
    only_empty: bool,

    /// Skip unfilled variables
    #[arg(short = 'f', long, default_value_t = false)]
    only_filled: bool,
}

pub async fn prompt(text: String) -> Result<Option<String>> {
    eprint!("{}: ", text);
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    // handle explicit EOF
    if line.is_empty() {
        return Ok(None);
    }
    while line.ends_with(|c: char| c.is_whitespace()) {
        line.pop();
    }
    Ok(Some(line))
}

impl Cmd {
    pub async fn run(self: Cmd) -> Result<()> {
        let input_path = Path::new(&self.input);
        let input = EnvFile::from_path(input_path)?;

        let output_path = Path::new(&self.output);
        if output_path.exists() && output_path.is_dir() {
            return Err(anyhow!("{} is a directory", output_path.to_string_lossy()));
        }
        let output = if output_path.exists() {
            let mut output = EnvFile::from_path(output_path)?;
            output.apply_assign(&input.env(), false);
            output
        } else {
            input.clone()
        };

        let mut output_env = output.env();
        let mut buffer = String::new();

        for row in input.stream() {
            let row = match row {
                EnvFileRow::Declaration(declaration) => declaration,
                EnvFileRow::CommentOnly(comment) => {
                    buffer += &format!("# {}\n", comment);
                    continue;
                }
                EnvFileRow::Empty => continue,
            };
            let key = row.name;
            let output_val = output_env.get(&key).cloned();
            if self.only_empty && output_val.is_some() {
                buffer.clear();
                continue;
            }
            if self.only_filled && output_val.is_none() {
                buffer.clear();
                continue;
            }
            let mut prompt_text = format!("{}{}", buffer, key);
            buffer.clear();
            let default = output_env.get(&key).cloned().unwrap_or("".to_string());
            if !default.is_empty() {
                prompt_text += &format!(" ({})", default);
            }
            let line = match prompt(prompt_text).await? {
                Some(line) if !line.is_empty() => line,
                Some(_) => default,
                None => {
                    eprintln!("(cleared)");
                    "".to_string()
                },
            };
            output_env.insert(key, line);
        }

        let mut output_file = if output_path.exists() {
            File::options().write(true).open(output_path)?
        } else {
            File::create(output_path)?
        };
        Ok(output_file.write_all(input.apply(&output_env, true).to_string().as_bytes())?)
    }
}

