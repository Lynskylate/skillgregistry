# Entities Development Guide

## IMPORTANT: Keep Migrations in Sync

**When you modify any entity in this directory, you MUST also update the corresponding migration.**

### Directory Structure

```
backend/
├── common/src/entities/     # Entity definitions (SeaORM models)
└── migration/src/           # Database migrations (SeaORM Migration)
```

### Workflow

When adding or modifying entities:

1. **Update Entity**: Modify the relevant `.rs` file in this directory
2. **Update Migration**: Add a new migration or modify existing migration in `backend/migration/src/`
3. **Run Tests**: Execute schema validation tests to ensure consistency:
   ```bash
   cd backend/migration
   cargo test
   ```

### What Needs Synchronization

Ensure these aspects match between entities and migrations:

- **Column names** - Must be identical (case-sensitive)
- **Data types** - Must be compatible
- **Nullability** - `Option<T>` in entity ↔ `nullable` in migration
- **Primary keys** - Marked with `#[sea_orm(primary_key)]` in entity
- **Unique constraints** - Marked with `#[sea_orm(unique)]` in entity
- **Default values** - Set in migration, reflected in entity if needed
- **Foreign keys** - Relations defined in entity, constraints in migration

### Schema Validation Tests

The project includes automatic schema validation tests that will fail if:
- A column exists in the entity but not in the database
- A column exists in the database but not in the entity
- Column types don't match
- Nullability constraints differ

**Location**: `backend/migration/tests/schema_validation_tests.rs`

### Example

If you add a new column to `skills` entity:

```rust
// In backend/common/src/entities/skills.rs
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "skills")]
pub struct Model {
    // ... existing fields
    pub category: Option<String>, // NEW FIELD
}
```

You must create a migration:

```rust
// In backend/migration/src/m20260206_000002_add_category_to_skills.rs
async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Skills::Table)
                .add_column(string_null(Skills::Category))
                .to_owned(),
        )
        .await
}
```

### Tips

1. **Always test locally** before committing:
   ```bash
   cd backend/migration
   cargo test test_all_tables_exist
   cargo test test_skills_schema_matches_entity
   ```

2. **Check existing tests** for reference on how entities and migrations should align

3. **Use SQLite for development** - Tests run against in-memory SQLite for speed

4. **When in doubt**, check the existing implementation in `m20260205_000001_create_all_tables.rs`

## Running Tests

```bash
# Run all migration tests
cd backend/migration
cargo test

# Run specific table validation
cargo test test_skill_registry_schema_matches_entity
cargo test test_users_schema_matches_entity
```

---

**Remember**: Keeping entities and migrations in sync is critical for application stability!
