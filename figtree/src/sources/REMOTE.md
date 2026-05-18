# Remote Sources — Future Work

This document captures the design plan for remote configuration sources
in figtree. Remote sources are out of scope for the initial release but
are a planned addition. This file exists so a future context session can
pick up the work without reconstructing the design from scratch.

## Why Remote Sources Are Deferred

The initial release focuses on correctness of the local source stack and
the proc macro. Remote sources introduce new concerns that would delay
the initial release without adding value to early adopters:

- Network fallibility distinct from "key not present"
- Async-first HTTP clients that conflict with the sync Source trait
- External service dependencies that complicate testing
- Authentication and credential management (Vault tokens, AWS IAM, etc.)

These concerns are solvable but require careful design. Deferring them
keeps the initial release focused and shippable.

## What Exists Today

The current Source trait in priority.rs is synchronous and infallible
from the caller's perspective:

    pub trait Source: Send + Sync {
        fn get(&self, key: &str) -> Option<FigValue>;
        fn source_name(&self) -> String;
        fn as_fig_source(&self) -> FigSource;
    }

None returns when a source has no value for a key. There is no way
to signal a network error distinct from key absence. This is correct
for local sources (file, env, cli) and incorrect for remote sources
where a timeout is meaningfully different from a missing key.

## Trait Changes Required

Two new traits need to be added to priority.rs alongside the existing
Source trait. The existing Source trait must not change — all current
implementations remain valid.

    /// For sources that can fail distinctly from key absence.
    /// Network errors, auth failures, and timeouts surface as Err
    /// rather than None.
    pub trait FallibleSource: Send + Sync {
        fn get(&self, key: &str) -> FigtreeResult<Option<FigValue>>;
        fn source_name(&self) -> String;
        fn as_fig_source(&self) -> FigSource;
    }

    /// For non-blocking remote reads. Requires the async feature flag.
    /// Uses async_trait until async fn in traits is stabilized.
    #[cfg(feature = "async")]
    #[async_trait::async_trait]
    pub trait AsyncSource: Send + Sync {
        async fn get(&self, key: &str) -> FigtreeResult<Option<FigValue>>;
        fn source_name(&self) -> String;
        fn as_fig_source(&self) -> FigSource;
    }

The Tree struct needs two new source collections alongside the existing
ones:

    fallible_sources: Vec<Box<dyn FallibleSource>>,

    #[cfg(feature = "async")]
    async_sources: Vec<Box<dyn AsyncSource>>,

The resolve_all() function needs to consult these after the existing
file_sources in PEMDAS order. FallibleSource errors should be logged
or recorded on the Fig rather than hard-failing the entire resolution —
a network blip should not prevent the application from starting if a
default is available.

## Planned Source Implementations

All remote sources live in a new subdirectory:

    figtree/src/sources/remote/
    ├── mod.rs          re-exports, feature gates
    ├── http.rs         HTTP/HTTPS polling source
    ├── etcd.rs         etcd v3 KV source
    ├── consul.rs       HashiCorp Consul KV source
    ├── vault.rs        HashiCorp Vault KV source
    ├── aws_ssm.rs      AWS Parameter Store + Secrets Manager
    ├── redis.rs        Redis GET + pub/sub live update source
    └── k8s.rs          Kubernetes ConfigMap and Secret source

### http.rs

Fetches a remote JSON, YAML, TOML, RON, or INI file over HTTP/HTTPS.
The URL is provided at construction time. The file is fetched once at
load/parse time and again on each pollinate() call.

Feature flag: remote-http
Crate dependency: reqwest (blocking feature for sync, full for async)

Constructor:

    HttpSource::load(url: &str) -> FigtreeResult<Self>
    HttpSource::load_async(url: &str) -> impl AsyncSource

The sync version uses reqwest::blocking. The async version implements
AsyncSource.

### etcd.rs

Reads from an etcd v3 cluster using the etcd-client crate. Keys map
directly to figtree key names. Values are stored as UTF-8 strings in
etcd and parsed via parse_env_value() using the hint type.

Feature flag: remote-etcd
Crate dependency: etcd-client

etcd's watch API maps naturally to figtree's mutation channel — a watch
goroutine (Rust: task) can push mutations directly without polling.
This is the ideal architecture for etcd and should be implemented.

Constructor:

    EtcdSource::connect(endpoints: &[&str]) -> FigtreeResult<Self>
    EtcdSource::connect_with_auth(endpoints, user, password) -> FigtreeResult<Self>

### consul.rs

Reads from HashiCorp Consul's KV store via its HTTP API. Consul has
a blocking query API that holds the connection open until a value
changes — this maps well to the mutation channel without polling.

Feature flag: remote-consul
Crate dependency: reqwest (Consul's API is REST)

Constructor:

    ConsulSource::new(address: &str, token: Option<&str>) -> Self

### vault.rs

Reads secrets from HashiCorp Vault's KV secrets engine (both v1 and v2).
Vault tokens have TTLs — the source must handle token renewal.
Secrets are concealed values — they should never appear in usage() output
or history logs. The Fig's Rule::NoCallbacks should be set automatically
for Vault-sourced keys to prevent secrets from leaking through callbacks.

Feature flag: remote-vault
Crate dependency: reqwest (Vault's API is REST)

This is the highest-priority remote source because figtree already has
concealed value semantics that align with Vault's purpose.

Constructor:

    VaultSource::new(address: &str, token: &str, mount: &str) -> Self
    VaultSource::new_with_approle(address, role_id, secret_id, mount) -> Self

### aws_ssm.rs

Reads from AWS Systems Manager Parameter Store and Secrets Manager.
Uses the AWS SDK for Rust. Supports both standard and SecureString
parameters. SecureString parameters are automatically decrypted using
the caller's IAM permissions.

Feature flag: remote-aws
Crate dependencies: aws-sdk-ssm, aws-sdk-secretsmanager, aws-config

Constructor:

    AwsSsmSource::new(region: &str) -> FigtreeResult<Self>
    AwsSsmSource::new_with_prefix(region, prefix) -> FigtreeResult<Self>

The prefix maps a path prefix in Parameter Store to the figtree
key namespace. For example prefix "/myapp/prod/" means the key
"workers" is fetched from "/myapp/prod/workers".

### redis.rs

Reads configuration values from Redis using GET. Also supports
pub/sub notification via SUBSCRIBE to a config change channel,
which maps directly to figtree's mutation channel without polling.

Feature flag: remote-redis
Crate dependency: redis

The pub/sub integration is the most interesting part. When a value
changes in Redis, the publisher sends a message on the config channel.
The RedisSource subscriber receives it, fetches the new value, updates
the Fig, and emits a Mutation. This gives true push-based live config
updates.

Constructor:

    RedisSource::new(url: &str) -> FigtreeResult<Self>
    RedisSource::new_with_prefix(url, prefix) -> FigtreeResult<Self>

### k8s.rs

Reads from Kubernetes ConfigMaps and Secrets via the kube-rs client.
Useful for applications running inside a Kubernetes cluster that want
to consume their own ConfigMaps without mounting them as files.

Feature flag: remote-k8s
Crate dependencies: kube, k8s-openapi

ConfigMap data keys map to figtree keys. Secret data is base64-decoded
automatically. The kube-rs watcher API maps to figtree's mutation channel.

Constructor:

    K8sSource::from_configmap(namespace: &str, name: &str) -> FigtreeResult<Self>
    K8sSource::from_secret(namespace: &str, name: &str) -> FigtreeResult<Self>

## Feature Flags To Add

Add these to figtree/Cargo.toml [features]:

    remote-http   = ["reqwest"]
    remote-etcd   = ["etcd-client", "async"]
    remote-consul = ["reqwest"]
    remote-vault  = ["reqwest"]
    remote-aws    = ["aws-sdk-ssm", "aws-sdk-secretsmanager", "aws-config", "async"]
    remote-redis  = ["redis", "async"]
    remote-k8s    = ["kube", "k8s-openapi", "async"]

Add these to workspace Cargo.toml [workspace.dependencies]:

    reqwest      = { version = "0.12", features = ["blocking", "json"] }
    etcd-client  = "0.14"
    redis        = { version = "0.25", features = ["tokio-comp", "connection-manager"] }
    kube         = { version = "0.88", features = ["runtime", "derive"] }
    k8s-openapi  = { version = "0.21", features = ["v1_28"] }
    aws-sdk-ssm              = "1"
    aws-sdk-secretsmanager   = "1"
    aws-config               = "1"

## Pollination Integration

Remote sources are where pollination becomes most valuable. The pattern
for a long-running service:

    let mut tree = Tree::with(Options {
        tracking:  true,
        pollinate: true,
        ..Options::default()
    });

    tree.with_remote_http_source("https://config.myapp.com/config.json")?;
    tree.parse()?;

    let rx = tree.mutations().unwrap();

    // react to config changes
    std::thread::spawn(move || {
        for mutation in rx.iter() {
            log::info!("config changed: {}", mutation);
        }
    });

    // re-check remote config every 60 seconds
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            tree.pollinate().unwrap();
        }
    });

For etcd, Consul, and Redis the polling loop is replaced by the source's
own watch/subscribe mechanism which pushes changes directly. The
pollinate() call is still valid for these sources but is a no-op when
the push mechanism is active.

## Concealed Values And Security

Remote sources frequently deliver secrets. Two things must be true
for figtree to handle secrets safely:

1. Concealed values must never appear in usage() output or history logs.
   A future Rule::Concealed should suppress the value in all diagnostic
   output and replace it with asterisks.

2. The Fig history for a concealed key should record that a change
   occurred and from which source, but not record the actual value.
   FigHistoryEntry needs a concealed: bool field that triggers this.

This is also deferred but should be designed alongside remote sources
since Vault and AWS Secrets Manager are the primary drivers.

## Testing Strategy

Remote sources cannot be unit tested against live services in CI.
The testing strategy is:

- Mock implementations of FallibleSource and AsyncSource for unit tests
- Docker Compose files in a tests/integration/ directory for local
  integration testing against real etcd, Consul, Vault, and Redis
- Conditional compilation in CI — remote source tests run only when
  the relevant service is available via environment variable flag

## Priority Order For Implementation

When this work begins, implement in this order:

    1. FallibleSource trait addition to priority.rs
    2. AsyncSource trait addition to priority.rs (async feature)
    3. Tree support for fallible_sources and async_sources
    4. http.rs — most broadly useful, no auth complexity
    5. vault.rs — highest security value, concealed values design
    6. aws_ssm.rs — large user base, AWS is common deployment target
    7. consul.rs — strong Kubernetes + HashiCorp ecosystem fit
    8. etcd.rs — Kubernetes native, watch API is elegant
    9. redis.rs — pub/sub live update is the most interesting feature
    10. k8s.rs — niche but natural for in-cluster applications
