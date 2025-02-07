# protobuf-nats-service-generator

> [!WARNING]
> This crate is experimental and likely to experience breaking changes.

This crate is responsible for generating `Server` and `Client` traits from protobuf service definitions, enabling simple client-side requests using [async-nats](https://crates.io/crates/async-nats) and implementing and handling server-side requests using a generated trait.

## Feature support

This crate aims to support all four kinds of [service definitions over NATS](https://grpc.io/docs/what-is-grpc/core-concepts/#service-definition). Currently it supports the following:

- [x] Single request/response RPCs
- [x] Server streaming RPCs
- [ ] Client streaming RPCs
- [ ] Bidirectional streaming RPCs

## Usage

Add this crate's `protobuf_nats_service_generator::NatsServiceGenerator` to your `build.rs` file as a service generator. See the [prost_build](https://docs.rs/prost-build/latest/prost_build/) crate for more information on generating Rust types from .proto files.

## Subject generation

This crate subscribes and sends requests on a generated subject of the following form: `{prefix}.{dot_delimited_rpc_name}`. The default prefix is `nats.proto`. So, for an example proto service:

```proto
syntax = "proto3";

package example;

// Define a simple message type
message Person {
    string first_name = 1;
    string last_name = 2;
    int32 age = 3;
}

// Define a request message
message GetPersonRequest {
    int32 id = 1;
}

// Define a response message
message GetPersonResponse {
    Person person = 1;
}

// Define the RPC service
service PersonService {
    rpc GetPerson(GetPersonRequest) returns (GetPersonResponse);
}
```

Services started using the generated `{name}Server` trait will subscribe on `nats.proto.>`. The generated `{name}Client` trait implementation for `async_nats::Client` will send requests on `nats.proto.get.person`, which will properly be received and protobuf decoded by the server.

### Overriding the default subject prefix

You can override the default subject prefix by disabling the `auto_subject_prefix` feature on this crate and adding an implementation for yourself. For example:

```rust
impl {name}ClientPrefix for ::async_nats::Client {
    fn subject_prefix(&self) -> &'static str {
        "my.custom.prefix"
    }
}
impl {name}Server for MyServiceType {
    fn subject_prefix(&self) -> &'static str {
        "my.custom.prefix"
    }
    // rest of the implementations
}
```

This function should be implemented in both the server and client implementations to ensure that they are communicating on the correct subjects.

## Example

You can see an example of using this crate under [examples/simple](./examples/simple/). Below is the generated code from that example.

````rust
// This file is @generated by prost-build.
/// --------------------------------------------------------------
/// This file was generated by the `protobuf-nats-service-generator` crate
/// DO NOT MODIFY DIRECTLY
/// --------------------------------------------------------------
use ::anyhow::Context as _;
use ::futures::StreamExt;
use ::prost::Message;
/// Define a simple message type
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Address {
    #[prost(string, tag = "1")]
    pub street: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub city: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub state: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub zip_code: ::prost::alloc::string::String,
}
/// Define another simple message type
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Person {
    #[prost(string, tag = "1")]
    pub first_name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub last_name: ::prost::alloc::string::String,
    #[prost(int32, tag = "3")]
    pub age: i32,
    #[prost(message, optional, tag = "4")]
    pub address: ::core::option::Option<Address>,
}
/// Define a request message
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct GetPersonRequest {
    #[prost(int32, tag = "1")]
    pub id: i32,
}
/// Define a response message
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetPersonResponse {
    #[prost(message, optional, tag = "1")]
    pub person: ::core::option::Option<Person>,
}
pub trait PersonServiceClientPrefix {
    /// Get the subject prefix for this service. Defaults to
    /// "nats.proto" and can be overridden with your own implementation.
    ///
    /// # Usage
    /// For the default prefix, enable the `auto_subject_feature` or implement the trait as:
    /// ```rust
    /// impl PersonServiceClientPrefix for async_nats::Client {}
    /// ```
    ///
    /// To use your own prefix, implement this trait for your client:
    /// ```rust
    /// impl PersonServiceClientPrefix for async_nats::Client {
    ///     fn subject_prefix(&self) -> &'static str {
    ///        "my.prefix"
    ///     }
    /// }
    /// ```
    fn subject_prefix(&self) -> &'static str {
        "nats.proto"
    }
}
/// This will be used to implement the handlers for the client
pub trait PersonServiceClient {
    /// Send request [GetPersonRequest], receiving the decoded [GetPersonResponse]
    #[allow(dead_code)]
    fn get_person(
        &self,
        _request: GetPersonRequest,
    ) -> impl ::futures::Future<Output = ::anyhow::Result<GetPersonResponse>> + Send;
}
/// Implement the PersonServiceClient trait for the async_nats::Client
///
/// # Usage
/// ```ignore
/// use generated::{RequestType, ResponseType, PersonServiceClientPrefix};
/// /// Define your subject prefix or use the default. The PersonServiceClient trait is already
/// /// implemented for the async_nats::Client, so you can use it directly.
/// impl PersonServiceClientPrefix for async_nats::Client {}
/// async fn main() -> anyhow::Result<()> {
///    let client = async_nats::connect("nats://127.0.0.1:4222").await.expect("to connect");
///    let request = RequestType::default();
///    let response: ResponseType = client.function_name(request).await.expect("to send request");
///    Ok(())
/// }
///
/// ```
impl PersonServiceClientPrefix for ::async_nats::Client {}
impl PersonServiceClient for ::async_nats::Client
where
    ::async_nats::Client: PersonServiceClientPrefix,
{
    /// Send request [GetPersonRequest], decode response as [GetPersonResponse]
    async fn get_person(
        &self,
        request: GetPersonRequest,
    ) -> anyhow::Result<GetPersonResponse> {
        let mut buf = ::bytes::BytesMut::with_capacity(request.encoded_len());
        request.encode(&mut buf).context("failed to encode GetPersonRequest")?;
        let reply = self
            .request(
                format!("{}.get.person", self.subject_prefix().trim_end_matches('.')),
                buf.into(),
            )
            .await
            .context("failed to send NATS request for get_person")?;
        GetPersonResponse::decode(reply.payload)
            .context("failed to decode reply as GetPersonResponse")
    }
}
/// This will be used to implement the handlers for the server
pub trait PersonServiceServer {
    /// Get the subject prefix for this service. Defaults to
    /// "nats.proto" and can be overridden with your own implementation.
    /// If the subject prefix does not include the trailing '.' character, it will be added.
    fn subject_prefix(&self) -> &'static str {
        "nats.proto"
    }
    /// Implementation of GetPerson
    fn get_person(
        &self,
        _request: GetPersonRequest,
    ) -> impl ::futures::Future<Output = ::anyhow::Result<GetPersonResponse>> + Send;
}
#[allow(dead_code)]
pub async fn start_server<S>(
    server: S,
    client: async_nats::Client,
) -> ::anyhow::Result<impl ::futures::Future<Output = ::anyhow::Result<()>>>
where
    S: PersonServiceServer + Send + 'static,
{
    let subject_prefix = server.subject_prefix().trim_end_matches('.');
    let mut subscription = client
        .subscribe(format!("{subject_prefix}.>"))
        .await
        .context("failed to subscribe for PersonService messages")?;
    Ok(async move {
        while let Some(message) = subscription.next().await {
            match message.subject.as_str().strip_prefix(&subject_prefix) {
                Some(".get.person") => {
                    let request = GetPersonRequest::decode(message.payload)
                        .context(
                            "failed to decode message payload as GetPersonRequest",
                        )?;
                    let reply = server
                        .get_person(request)
                        .await
                        .context("failed to handle GetPersonRequest request")?;
                    if let Some(reply_to) = message.reply {
                        let mut buf = ::bytes::BytesMut::with_capacity(
                            reply.encoded_len(),
                        );
                        reply
                            .encode(&mut buf)
                            .expect("to encode without reaching capacity");
                        client
                            .publish(reply_to, buf.into())
                            .await
                            .context("failed to publish reply")?;
                    } else {
                        eprintln!("No reply subject found in message");
                    }
                }
                _ => {
                    eprintln!(
                        "received message on unknown subject: {}", message.subject
                    );
                }
            }
        }
        Ok(())
    })
}

````

## Credit

Huge thanks to the [protobuf-zmq-rust-generator](https://crates.io/crates/protobuf-zmq-rust-generator) crate for inspiration and instructions.
