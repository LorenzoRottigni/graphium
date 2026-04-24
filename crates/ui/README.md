# Graphium UI

Graphium UI is an interactive web-based playground for visualizing, testing, and inspecting Graphium workflows.

This crate provides a comprehensive interface for exploring graph structure, executing graphs with custom contexts, viewing real-time metrics, and running test suites. The UI leverages Mermaid diagrams for graph visualization and offers a developer-friendly playground for experimentation.

Built with modern web technologies, the UI runs as a local server that connects to your Rust crate, providing instant feedback and deep insights into graph behavior and performance.

## Features

- **Graph Visualization**: Interactive Mermaid diagrams showing workflow structure and artifact flow
- **Manual Execution**: Run graphs with custom context values and inspect outputs
- **Metrics Dashboard**: Real-time tracking of performance metrics, error rates, and execution statistics
- **Node Inspection**: Deep dive into individual node behavior, inputs, and outputs
- **Test Interface**: Browse and execute graph and node tests with detailed result reporting
- **Real-Time Playground**: Experiment with graph configurations and see results immediately
- **Schema Explorer**: View complete graph schema including node contracts and artifact types

## Getting Started

### Prerequisites

- Rust toolchain
- cargo-watch for development

### Running Locally

1. Install cargo-watch:

```bash
cargo install cargo-watch
```

2. Start the development server with hot-reload:

```bash
cargo watch -x "run -p graphium-examples --bin graphium_ui"
```

3. Open your browser to `http://localhost:3000` (or the port shown in terminal output)

### Configuration

The UI server can be configured via environment variables. See `src/config.rs` for available options.

## Architecture

- **Server** (`src/server.rs`): Axum-based HTTP server
- **Templates** (`templates/`): HTML templates for UI pages
- **Styling** (`assets/css/`): CSS for visual presentation
- **Mermaid Integration** (`src/mermaid.rs`): Graph diagram generation
- **Metrics** (`src/metrics.rs`): Real-time metrics collection and reporting

## Development

The UI is built with:
- **Backend**: Axum web framework
- **Frontend**: Server-rendered HTML with vanilla JavaScript
- **Diagrams**: Mermaid for workflow visualization

## Documentation

See [Graphium](https://github.com/rottigni/graphium) for comprehensive documentation and examples.
