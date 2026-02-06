* Don't depend on the `sea_orm::DatabaseConnection` directly; you should depend on the `Repository` interface.
* The worker should only depend on the `Repository` interface, not on the concrete implementation.
* You should use the service layer to handle the business logic.