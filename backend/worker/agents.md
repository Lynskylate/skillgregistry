* Don't depend on the seaorm:DatabaseConnection directly, you should depends on the Repository interface.
* The worker should only depends on the Repository interface, not the concrete implementation.
* You should use the service layer to handle the business logic.