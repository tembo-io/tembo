use crate::cli::context::list_context;
use crate::tui::{colors::sql_u, indent, label};
use cli_table::{Cell, CellStruct, Style, Table};
use colorful::Colorful;

pub fn execute() -> Result<(), anyhow::Error> {
    let context = list_context()?;

    let mut rows: Vec<Vec<CellStruct>> = vec![];
    for e in context.environment {
        let mut org_id = String::from("           ");
        let mut profile = String::from("   ");
        let mut set = false;
        if let Some(env_org) = e.org_id {
            org_id = env_org;
        }

        if e.target == *"docker" {
            profile = String::from("local")
        } else if let Some(env_profile) = e.profile {
            profile = env_profile;
        }

        if let Some(env_set) = e.set {
            set = env_set;
        }

        rows.push(vec![
            e.name.cell(),
            e.target.cell(),
            org_id.cell(),
            profile.cell(),
            set.cell(),
        ]);
    }

    let table = rows
        .table()
        .title(vec![
            "Name".color(sql_u()).cell().bold(true),
            "Target".color(sql_u()).cell().bold(true),
            "Org ID".color(sql_u()).cell().bold(true),
            "Profile".color(sql_u()).cell().bold(true),
            "Set".color(sql_u()).cell().bold(true),
        ])
        .bold(true);

    let table_display = table
        .display()
        .expect("Error: could not parse `tembo context list` table contents!");

    label("Your current Tembo context:");
    println!("{}", indent(1));
    println!("{}", table_display);

    Ok(())
}
