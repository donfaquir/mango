use std::io::{self, Write};

use clap::{Args, Subcommand};
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};
use rusqlite::Connection;

use mango_core::db::queries::project as project_queries;
use mango_core::models::project::{CreateProjectInput, ListProjectsOptions};

#[derive(Args)]
pub struct ProjectArgs {
    #[command(subcommand)]
    action: ProjectAction,
}

#[derive(Subcommand)]
enum ProjectAction {
    /// Create a new project
    Create {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, default_value = "")]
        style_prompt: String,
    },
    /// List all projects
    List,
    /// Show project details by ID
    Get { id: String },
    /// Delete a project by ID
    Delete {
        id: String,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(conn: &Connection, args: ProjectArgs) -> anyhow::Result<()> {
    match args.action {
        ProjectAction::Create {
            name,
            description,
            style_prompt,
        } => create(conn, name, description, style_prompt),
        ProjectAction::List => list(conn),
        ProjectAction::Get { id } => get(conn, &id),
        ProjectAction::Delete { id, yes } => delete(conn, &id, yes),
    }
}

fn create(
    conn: &Connection,
    name: String,
    description: String,
    style_prompt: String,
) -> anyhow::Result<()> {
    let project = project_queries::create(
        conn,
        CreateProjectInput {
            name,
            description: Some(description),
            style_prompt: Some(style_prompt),
            global_seed: None,
        },
    )?;
    println!("Project created");
    println!("  ID:   {}", project.id);
    println!("  Name: {}", project.name);
    Ok(())
}

fn list(conn: &Connection) -> anyhow::Result<()> {
    let projects = project_queries::list(conn, ListProjectsOptions::default())?;

    if projects.is_empty() {
        println!(
            "No projects found. Create one with: \
             mango project create --name \"My Project\""
        );
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["ID", "Name", "Description", "Created"]);

    for p in &projects {
        table.add_row(vec![
            short_id(&p.id),
            p.name.clone(),
            truncate(&p.description, 30),
            p.created_at.clone(),
        ]);
    }

    println!("{table}");
    println!("\n{} project(s) total", projects.len());
    Ok(())
}

fn get(conn: &Connection, id: &str) -> anyhow::Result<()> {
    let project = project_queries::get_by_id(conn, id)?;
    println!("ID:          {}", project.id);
    println!("Name:        {}", project.name);
    println!("Description: {}", project.description);
    println!("Style:       {}", project.style_prompt);
    if let Some(seed) = project.global_seed {
        println!("Seed:        {seed}");
    }
    println!("Created:     {}", project.created_at);
    println!("Updated:     {}", project.updated_at);
    Ok(())
}

fn delete(conn: &Connection, id: &str, skip_confirm: bool) -> anyhow::Result<()> {
    if !skip_confirm {
        eprint!("Delete project {}? [y/N] ", short_id(id));
        io::stderr().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    project_queries::delete(conn, id)?;
    println!("Project deleted");
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
