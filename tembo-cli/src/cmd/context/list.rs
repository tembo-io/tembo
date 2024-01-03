use crate::cli::context::list_context;
use cli_table::{Cell, Style, Table, CellStruct};

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

        if e.target == String::from("docker") {
            profile = String::from("local")
        } else if let Some(env_profile) = e.profile {
            profile = env_profile;
        }

        if let Some(env_set) = e.set {
            set = env_set;
        }

        rows.push(vec![e.name.cell(), e.target.cell(), org_id.cell(), profile.cell(), set.cell()]);
    }

    let table = rows
    .table()
    .title(vec![
        "Name".cell().bold(true),
        "Target".cell().bold(true),
        "Org ID".cell().bold(true),
        "Profile".cell().bold(true),
        "Set".cell().bold(true),
    ])
    .bold(true);

    let table_display = table.display().expect("Error: could not parse `tembo context list` table contents!");

    println!("{}", table_display);

    Ok(())
}
