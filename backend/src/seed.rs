use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, Set,
    TransactionTrait,
};
use serde::Deserialize;
use std::collections::HashSet;

use beerio_kart::drink_type_id::drink_type_uuid;
use beerio_kart::entities::{bodies, characters, cups, drink_types, gliders, tracks, wheels};

// Serde structs matching the JSON file shapes. These are separate from the
// SeaORM entities because entity Models carry ORM metadata we don't need for
// deserialization, and keeping them separate avoids coupling seed data format
// to the ORM layer.

#[derive(Deserialize)]
struct SeedItem {
    id: i32,
    name: String,
    image_path: String,
}

#[derive(Deserialize)]
struct SeedTrack {
    id: i32,
    name: String,
    cup_id: i32,
    position: i32,
    image_path: String,
}

/// Load seed data into empty tables. Skips tables that already have rows.
/// All inserts for a given table happen inside a single transaction.
pub async fn run(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    seed_simple_table::<cups::Entity, cups::ActiveModel>(
        db,
        "cups",
        include_str!("../../data/cups.json"),
    )
    .await?;

    seed_simple_table::<characters::Entity, characters::ActiveModel>(
        db,
        "characters",
        include_str!("../../data/characters.json"),
    )
    .await?;

    seed_simple_table::<bodies::Entity, bodies::ActiveModel>(
        db,
        "bodies",
        include_str!("../../data/bodies.json"),
    )
    .await?;

    seed_simple_table::<wheels::Entity, wheels::ActiveModel>(
        db,
        "wheels",
        include_str!("../../data/wheels.json"),
    )
    .await?;

    seed_simple_table::<gliders::Entity, gliders::ActiveModel>(
        db,
        "gliders",
        include_str!("../../data/gliders.json"),
    )
    .await?;

    // Tracks depend on cups (FK), so they come after cups.
    seed_tracks(db).await?;

    // Drink types seeded with deterministic UUIDs (same logic as the API).
    seed_drink_types(db).await?;

    Ok(())
}

/// Seed a table that has the simple (id, name, image_path) schema.
/// Skips if the table already has data.
async fn seed_simple_table<E, A>(
    db: &DatabaseConnection,
    table_name: &str,
    json_data: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    E: EntityTrait,
    A: ActiveModelBehavior<Entity = E> + From<SimpleActiveModel> + Send,
    <E as EntityTrait>::Model: IntoActiveModel<A>,
{
    let existing = E::find().one(db).await?;
    if existing.is_some() {
        tracing::debug!("{table_name}: already seeded, skipping");
        return Ok(());
    }

    let items: Vec<SeedItem> = serde_json::from_str(json_data)?;
    let num_items = items.len();

    let txn = db.begin().await?;
    for item in items {
        let model: A = SimpleActiveModel {
            id: item.id,
            name: item.name,
            image_path: item.image_path,
        }
        .into();
        model.insert(&txn).await?;
    }
    txn.commit().await?;

    tracing::info!("{table_name}: seeded {num_items} rows");
    Ok(())
}

/// Intermediate struct for converting JSON data into any simple ActiveModel.
/// Each simple entity (characters, bodies, wheels, gliders, cups) implements
/// From<SimpleActiveModel> via the macro below.
struct SimpleActiveModel {
    id: i32,
    name: String,
    image_path: String,
}

macro_rules! impl_simple_seed {
    ($($module:ident),+) => {
        $(
            impl From<SimpleActiveModel> for $module::ActiveModel {
                fn from(s: SimpleActiveModel) -> Self {
                    Self {
                        id: Set(s.id),
                        name: Set(s.name),
                        image_path: Set(s.image_path),
                    }
                }
            }
        )+
    };
}

impl_simple_seed!(characters, bodies, wheels, gliders, cups);

async fn seed_tracks(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    let existing = tracks::Entity::find().one(db).await?;
    if existing.is_some() {
        tracing::debug!("tracks: already seeded, skipping");
        return Ok(());
    }

    let json_data = include_str!("../../data/tracks.json");
    let items: Vec<SeedTrack> = serde_json::from_str(json_data)?;

    // Validate that every track's cup_id references an existing cup.
    // We read cup IDs from the database (not just the JSON) so this catches
    // issues even if cups were seeded in a prior run.
    let cup_ids: HashSet<i32> = cups::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|c| c.id)
        .collect();

    for track in &items {
        if !cup_ids.contains(&track.cup_id) {
            return Err(format!(
                "Track '{}' (id={}) references cup_id={} which doesn't exist",
                track.name, track.id, track.cup_id
            )
            .into());
        }
    }

    let num_items = items.len();
    let txn = db.begin().await?;
    for track in items {
        let model = tracks::ActiveModel {
            id: Set(track.id),
            name: Set(track.name),
            cup_id: Set(track.cup_id),
            position: Set(track.position),
            image_path: Set(track.image_path),
        };
        model.insert(&txn).await?;
    }
    txn.commit().await?;

    tracing::info!("tracks: seeded {num_items} rows");
    Ok(())
}

#[derive(Deserialize)]
struct SeedDrinkType {
    name: String,
    alcoholic: bool,
}

async fn seed_drink_types(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    let existing = drink_types::Entity::find().one(db).await?;
    if existing.is_some() {
        tracing::debug!("drink_types: already seeded, skipping");
        return Ok(());
    }

    let json_data = include_str!("../../data/drink_types.json");
    let items: Vec<SeedDrinkType> = serde_json::from_str(json_data)?;
    let num_items = items.len();

    let now = chrono::Utc::now().to_rfc3339();
    let txn = db.begin().await?;
    for item in items {
        let id = drink_type_uuid(&item.name);
        let model = drink_types::ActiveModel {
            id: Set(id),
            name: Set(item.name),
            alcoholic: Set(item.alcoholic),
            created_at: Set(now.clone()),
            created_by: Set(None), // Pre-seeded entries have no creator
        };
        model.insert(&txn).await?;
    }
    txn.commit().await?;

    tracing::info!("drink_types: seeded {num_items} rows");
    Ok(())
}
