# Vault Keeper Service

A  backend service for vault keeper operations and basic API functionality. This service provides the foundation for keeper bots and a REST API.

## Overview

This is a  Rust service that includes:
- **Keeper Service**: Framework for automated vault operations
- **REST API**: Basic HTTP endpoints for health checks and placeholder functionality

## Features

### 🤖 **Keeper Service (Skeleton)**
- Framework for automated operations
- Ready for yield distribution implementation
- Placeholder for gas optimization and transaction management

### 🌐 **REST API (Basic)**
- Health check endpoint
- Placeholder user portfolio endpoint
- Placeholder vault stats endpoint

## Architecture

```
┌─────────────────┐    ┌─────────────────┐
│   Keeper        │    │   REST API      │
│   Service       │    │   Server        │
└─────────────────┘    └─────────────────┘
         │                       │
         └───────────────────────┘
                 │
         ┌───────────────┐
         │  Database     │
         │  (Future)     │
         └───────────────┘
```

## Quick Start

### Prerequisites

- Rust 1.70+

### Installation

1. **Clone and setup**
   ```bash
   git clone <repository>
   cd vault-keeper
   ```

2. **Run the service**
   ```bash
   cargo run
   ```

The service will start and display the skeleton structure with placeholders for future implementation.

## API Documentation

### Available Endpoints

#### Health Check
```http
GET /health
```
Returns service health status.

#### User Portfolio (Placeholder)
```http
GET /api/v1/users/{address}/portfolio
```
Currently returns a TODO message. Ready for implementation.

#### Vault Stats (Placeholder)
```http
GET /api/v1/vaults/stats
```
Currently returns a TODO message. Ready for implementation.

## Configuration

Currently the service uses a basic configuration setup. Environment variables and detailed configuration will be added as features are implemented.

## Development

### Running Tests
```bash
cargo test
```

### Code Structure

```
src/
├── main.rs              # Application entry point
├── config.rs            # Configuration management (skeleton)
├── database.rs          # Database layer (skeleton)
├── keeper.rs            # Keeper service (skeleton)
└── api.rs               # REST API server (basic implementation)
```

### Adding New Features

This is a skeleton project ready for implementation. Key areas to develop:

1. **Database Integration**: Implement database connectivity in `database.rs`
2. **Configuration**: Add environment variable handling in `config.rs`
3. **Keeper Logic**: Implement automated operations in `keeper.rs`
4. **API Endpoints**: Expand REST API functionality in `api.rs`

## Next Steps

This skeleton provides the foundation for building a comprehensive vault keeper service. Implement the TODO items in each module to build out the full functionality.

### Priority Implementation Areas

1. **Database Layer**: Set up PostgreSQL integration and schema
2. **Configuration Management**: Environment variables and settings
3. **Keeper Operations**: Automated yield distribution and monitoring
4. **API Expansion**: Full REST API with authentication and data endpoints

## License

[MIT License](LICENSE)
