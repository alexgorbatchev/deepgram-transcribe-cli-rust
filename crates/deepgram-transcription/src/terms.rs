use crate::{Error, io};
use std::{collections::HashSet, path::Path};

pub fn parse<'a>(raw: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    raw.into_iter()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn load(path: &Path) -> Result<Vec<String>, Error> {
    let data = std::fs::read_to_string(path)
        .map_err(|error| io(format!("reading terms file {}", path.display()), error))?;
    Ok(parse(
        data.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#')),
    ))
}

pub fn combine(lists: impl IntoIterator<Item = Vec<String>>) -> Vec<String> {
    let mut seen = HashSet::new();
    lists
        .into_iter()
        .flatten()
        .map(|term| term.trim().to_owned())
        .filter(|term| !term.is_empty() && seen.insert(term.to_lowercase()))
        .collect()
}

pub fn tech_terms() -> Vec<String> {
    [
        "Go",
        "Golang",
        "TypeScript",
        "JavaScript",
        "Python",
        "Rust",
        "C++",
        "Java",
        "Kotlin",
        "Swift",
        "Node.js",
        "Bun",
        "Deno",
        "React",
        "Vue",
        "Next.js",
        "Svelte",
        "Angular",
        "Tailwind",
        "Vite",
        "WebAssembly",
        "Kubernetes",
        "K8s",
        "Docker",
        "AWS",
        "GCP",
        "Azure",
        "Terraform",
        "Helm",
        "Serverless",
        "Lambda",
        "Cloudflare",
        "Microservices",
        "CI/CD",
        "GitHub Actions",
        "Envoy",
        "Nginx",
        "Istio",
        "PostgreSQL",
        "Postgres",
        "MySQL",
        "Redis",
        "MongoDB",
        "SQLite",
        "Cassandra",
        "DynamoDB",
        "Kafka",
        "RabbitMQ",
        "Vector DB",
        "Elasticsearch",
        "ClickHouse",
        "REST",
        "GraphQL",
        "gRPC",
        "Protobuf",
        "WebSockets",
        "OAuth",
        "JWT",
        "Distributed Systems",
        "Event-Driven",
        "Concurrency",
        "Goroutine",
        "Channel",
        "Garbage Collection",
        "Scalability",
        "Observability",
        "OpenTelemetry",
        "Prometheus",
        "Grafana",
        "Tracing",
        "Metrics",
        "Unit Test",
        "Integration Test",
        "Agile",
        "Scrum",
        "System Design",
        "Code Review",
        "GitOps",
        "Rate Limiting",
        "Circuit Breaker",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
