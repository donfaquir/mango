use std::io::{self, Write};

use clap::{Args, Subcommand};
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};
use rusqlite::Connection;

use mango_core::db::queries::character as character_queries;
use mango_core::models::character::{
    CreateCharacterInput, ListCharactersOptions, UpdateCharacterInput,
};

#[derive(Args)]
pub struct CharacterArgs {
    #[command(subcommand)]
    action: CharacterAction,
}

#[derive(Subcommand)]
enum CharacterAction {
    /// List characters under a project
    List {
        #[arg(long)]
        project_id: String,
    },
    /// Create a new character
    Create {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, default_value = "")]
        appearance_prompt: String,
    },
    /// Show character details by ID
    Get { id: String },
    /// Update one or more fields
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        appearance_prompt: Option<String>,
    },
    /// Delete a character. Schema CASCADE removes its costumes.
    Delete {
        id: String,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(conn: &Connection, args: CharacterArgs) -> anyhow::Result<()> {
    match args.action {
        CharacterAction::List { project_id } => list(conn, &project_id),
        CharacterAction::Create {
            project_id,
            name,
            description,
            appearance_prompt,
        } => create(conn, project_id, name, description, appearance_prompt),
        CharacterAction::Get { id } => get(conn, &id),
        CharacterAction::Update {
            id,
            name,
            description,
            appearance_prompt,
        } => update(conn, &id, name, description, appearance_prompt),
        CharacterAction::Delete { id, yes } => delete(conn, &id, yes),
    }
}

fn create(
    conn: &Connection,
    project_id: String,
    name: String,
    description: String,
    appearance_prompt: String,
) -> anyhow::Result<()> {
    let c = character_queries::create(
        conn,
        CreateCharacterInput {
            project_id,
            name,
            description: Some(description),
            appearance_prompt: Some(appearance_prompt),
            reference_image_path: None,
            voice_id: None,
        },
    )?;
    println!("Character created");
    println!("  ID:   {}", c.id);
    println!("  Name: {}", c.name);
    Ok(())
}

fn list(conn: &Connection, project_id: &str) -> anyhow::Result<()> {
    let rows = character_queries::list(
        conn,
        ListCharactersOptions {
            project_id: project_id.to_string(),
            limit: None,
            offset: None,
        },
    )?;
    if rows.is_empty() {
        println!("No characters under project {}", short_id(project_id));
        return Ok(());
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["ID", "Name", "Description", "Created"]);
    for c in &rows {
        table.add_row(vec![
            short_id(&c.id),
            c.name.clone(),
            truncate(&c.description, 30),
            c.created_at.clone(),
        ]);
    }
    println!("{table}");
    println!("\n{} character(s) total", rows.len());
    Ok(())
}

fn get(conn: &Connection, id: &str) -> anyhow::Result<()> {
    let c = character_queries::get_by_id(conn, id)?;
    println!("ID:               {}", c.id);
    println!("Project:          {}", c.project_id);
    println!("Name:             {}", c.name);
    println!("Description:      {}", c.description);
    println!("Appearance:       {}", c.appearance_prompt);
    if let Some(ref p) = c.reference_image_path {
        println!("Reference image:  {p}");
    }
    println!("Created:          {}", c.created_at);
    println!("Updated:          {}", c.updated_at);
    Ok(())
}

fn update(
    conn: &Connection,
    id: &str,
    name: Option<String>,
    description: Option<String>,
    appearance_prompt: Option<String>,
) -> anyhow::Result<()> {
    let c = character_queries::update(
        conn,
        id,
        UpdateCharacterInput {
            name,
            description,
            appearance_prompt,
            reference_image_path: None,
            voice_id: None,
        },
    )?;
    println!("Character updated");
    println!("  ID:   {}", c.id);
    println!("  Name: {}", c.name);
    Ok(())
}

fn delete(conn: &Connection, id: &str, skip_confirm: bool) -> anyhow::Result<()> {
    if !skip_confirm {
        eprint!(
            "Delete character {} (and all its costumes)? [y/N] ",
            short_id(id)
        );
        io::stderr().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    character_queries::delete(conn, id)?;
    println!("Character deleted");
    Ok(())
}

fn short_id(id: &str) -> String {
    let n = 8.min(id.len());
    id[..n].to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
    format!("{truncated}...")
}
