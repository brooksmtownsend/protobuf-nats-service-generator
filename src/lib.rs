use convert_case::{Case, Casing};
use prost_build::{Service, ServiceGenerator};

pub struct NatsServiceGenerator;

impl ServiceGenerator for NatsServiceGenerator {
    fn generate(&mut self, service: Service, buf: &mut String) {
        let client_handlers_trait = get_client_handlers_trait(&service);
        let client_nats_implementation = get_client_nats_implementation(&service);

        let server_handlers_trait = get_server_handlers_trait(&service);
        let server_nats_implementation = get_server_nats_implementation(&service);

        let code = format!(
            r#"
            // Client handlers
            {client_handlers_trait}
            {client_nats_implementation}
            // Server handlers
            {server_handlers_trait}
            {server_nats_implementation}
            "#
        );
        buf.push_str(&code);
    }

    fn finalize(&mut self, _buf: &mut String) {
        const IMPORTS_CODE: &str = r#"
/// --------------------------------------------------------------
/// This file was generated by the `protobuf-nats-service-generator` crate
/// DO NOT MODIFY DIRECTLY
/// --------------------------------------------------------------
use ::anyhow::Context as _;
use ::futures::StreamExt;
use ::prost::Message;
"#;
        _buf.insert_str(0, IMPORTS_CODE);
    }
}

/// Filter methods, returning methods of the provided [MethodType]
fn filter_methods_by_type(
    methods: &[prost_build::Method],
    desired_type: MethodType,
) -> Vec<&prost_build::Method> {
    methods
        .iter()
        .filter(|&method| {
            let method_type = get_method_type(method);
            method_type == desired_type
        })
        .collect()
}

/// Generate function handlers for client implementations of a [Service]
fn get_client_handlers_trait(service: &Service) -> String {
    let name = &service.name;

    let methods = &service.methods;
    let reply_methods = filter_methods_by_type(methods, MethodType::RequestResponse);
    let function_handlers = reply_methods
        .iter()
        .map(|method| {
            let function_name = convert_method_to_function(&method.name);
            // TODO: Support client side streaming
            let input_type = &method.input_type;
            let output_type = if method.server_streaming {
                format!("impl ::futures::Future<Output = ::anyhow::Result<::std::pin::Pin<::std::boxed::Box<impl ::futures::Stream<Item = {}>>>>>", method.output_type)
            } else {
                format!("impl ::futures::Future<Output = ::anyhow::Result<{}>>", method.output_type)
            };
            format!(
                r#"
                /// Send request [{input_type}], receiving the decoded [{output_type}]
                #[allow(dead_code)]
                fn {function_name}(
                    &self,
                    _request: {input_type},
                ) -> {output_type} + Send;
            "#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"
        pub trait {name}ClientPrefix {{
            /// Get the subject prefix for this service. Defaults to
            /// "nats.proto" and can be overridden with your own implementation.
            /// 
            /// # Usage
            /// For the default prefix, enable the `auto_subject_feature` or implement the trait as:
            /// ```rust
            /// impl {name}ClientPrefix for async_nats::Client {{}}
            /// ```
            /// 
            /// To use your own prefix, implement this trait for your client:
            /// ```rust
            /// impl {name}ClientPrefix for async_nats::Client {{
            ///     fn subject_prefix(&self) -> &'static str {{
            ///        "my.prefix"
            ///     }}
            /// }}
            /// ```
            fn subject_prefix(&self) -> &'static str {{
                "nats.proto"
            }}
        }}
        /// This will be used to implement the handlers for the client
        pub trait {name}Client {{
            {function_handlers}
        }}
        "#,
    )
}

/// Implement the [Service] trait for an [async_nats::Client]
fn get_client_nats_implementation(service: &Service) -> String {
    let name = &service.name;

    let methods = &service.methods;
    let reply_methods = filter_methods_by_type(methods, MethodType::RequestResponse);
    let functions = reply_methods
        .iter()
        .map(|method| {
            let function_name = convert_method_to_function(&method.name);
            let function_subject = convert_method_to_subject(&method.name);
            // TODO: Support client side streaming
            let input_type = &method.input_type;
            if method.server_streaming {
                format!(
                    r#"
                    /// Send request [{input_type}], decode response as a stream of [{output_type}]
                    async fn {function_name}(
                        &self,
                        request: {input_type},
                    ) -> ::anyhow::Result<::std::pin::Pin<::std::boxed::Box<impl ::futures::Stream<Item = {output_type}>>>> {{
                        let mut buf = ::bytes::BytesMut::with_capacity(request.encoded_len());
                        request
                            .encode(&mut buf)
                            .context("failed to encode {input_type}")?;
                        let inbox = self.new_inbox();
                        let sub = self.subscribe(inbox.clone()).await?;
                        self
                            .publish_with_reply(format!("{{}}.{function_subject}", self.subject_prefix().trim_end_matches('.')), inbox, buf.into())
                            .await?;
                        // TODO: provide error logging for failed decodes
                        let sub = sub.filter_map(|msg| async {{ {output_type}::decode(msg.payload).ok() }});

                        Ok(Box::pin(sub))
                    }}
                "#, output_type = method.output_type)
            } else {
                format!(
                    r#"
                    /// Send request [{input_type}], decode response as [{output_type}]
                    async fn {function_name}(
                        &self,
                        request: {input_type},
                    ) -> anyhow::Result<{output_type}> {{
                        let mut buf = ::bytes::BytesMut::with_capacity(request.encoded_len());
                        request
                            .encode(&mut buf)
                            .context("failed to encode {input_type}")?;
                        let reply = self
                            .request(format!("{{}}.{function_subject}", self.subject_prefix().trim_end_matches('.')), buf.into())
                            .await
                            .context("failed to send NATS request for {function_name}")?;
                        // TODO: more error handling on response message
                        {output_type}::decode(reply.payload).context("failed to decode reply as {output_type}")
                    }}
                "#, output_type = method.output_type)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // If the feature is enabled, generate the default implementation for the client prefix
    #[cfg(feature = "auto_subject_prefix")]
    let client_prefix_impl = format!("impl {name}ClientPrefix for ::async_nats::Client {{}}");
    #[cfg(not(feature = "auto_subject_prefix"))]
    let client_prefix_impl = "";

    format!(
        r#"
        /// Implement the {name}Client trait for the async_nats::Client
        /// 
        /// # Usage
        /// ```ignore
        /// use generated::{{RequestType, ResponseType, {name}ClientPrefix}};
        /// /// Define your subject prefix or use the default. The {name}Client trait is already
        /// /// implemented for the async_nats::Client, so you can use it directly.
        /// impl {name}ClientPrefix for async_nats::Client {{}}
        /// async fn main() -> anyhow::Result<()> {{
        ///    let client = async_nats::connect("nats://127.0.0.1:4222").await.expect("to connect");
        ///    let request = RequestType::default();
        ///    let response: ResponseType = client.function_name(request).await.expect("to send request");
        ///    Ok(())
        /// }}
        /// 
        /// ```
        {client_prefix_impl}
        impl {name}Client for ::async_nats::Client where ::async_nats::Client: {name}ClientPrefix {{
            {functions}
        }}
        "#
    )
}

/// Generate the trait for the handlers of a [Service]
fn get_server_handlers_trait(service: &Service) -> String {
    let name = &service.name;

    let methods = &service.methods;
    let reply_methods = filter_methods_by_type(methods, MethodType::RequestResponse);
    let function_handlers = reply_methods
        .iter()
        .map(|method| {
            let function_name = convert_method_to_function(&method.name);
            let method_name = &method.proto_name;
            let input_type = if method.client_streaming {
                format!("impl ::futures::Stream<Item = {}>", method.input_type)
            } else {
                method.input_type.to_string()
            };
            let output_type = if method.server_streaming {
                format!("impl ::futures::Future<Output = ::anyhow::Result<impl ::futures::Stream<Item = {}>>>", method.output_type)
            } else {
                format!("impl ::futures::Future<Output = ::anyhow::Result<{}>>", method.output_type)
            };
            format!(
                r#"
                /// Implementation of {method_name}
                fn {function_name}(
                    &self,
                    _request: {input_type},
                ) -> {output_type} + Send;
            "#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"
        /// This will be used to implement the handlers for the server
        pub trait {name}Server {{
            /// Get the subject prefix for this service. Defaults to
            /// "nats.proto" and can be overridden with your own implementation.
            /// If the subject prefix does not include the trailing '.' character, it will be added.
            fn subject_prefix(&self) -> &'static str {{
                "nats.proto"
            }}
            {function_handlers}
        }}
        "#,
    )
}

/// Create NATS subscriptions for a [Service] trait
fn get_server_nats_implementation(service: &Service) -> String {
    let name = &service.name;

    let methods = &service.methods;
    let reply_methods = filter_methods_by_type(methods, MethodType::RequestResponse);
    let matchy = reply_methods
        .iter()
        .map(|method| {
            let function_subject = convert_method_to_subject(&method.name);
            let function_name = convert_method_to_function(&method.name);
            let input_type = &method.input_type;
            if method.server_streaming {
                format!(
                    r#"
                    Some(".{function_subject}") => {{
                        let request = {input_type}::decode(message.payload)
                            .context("failed to decode message payload as {input_type}")?;
                        let replies = server
                            .{function_name}(request)
                            .await
                            .context("failed to handle {input_type} request")?;
                        ::futures::pin_mut!(replies);
                        if let Some(reply_to) = message.reply {{
                            while let Some(reply) = replies.next().await {{
                                let mut buf = ::bytes::BytesMut::with_capacity(reply.encoded_len());
                                reply
                                    .encode(&mut buf)
                                    .expect("to encode without reaching capacity");
                                client
                                    .publish(reply_to.clone(), buf.into())
                                    .await
                                    .context("failed to publish reply")?;
                            }}
                        }} else {{
                            eprintln!("No reply subject found in message");
                        }}
                    }},
                "#
                )
            } else {
                format!(
                    r#"
                    Some(".{function_subject}") => {{
                        let request = {input_type}::decode(message.payload)
                            .context("failed to decode message payload as {input_type}")?;
                        let reply = server
                            .{function_name}(request)
                            .await
                            .context("failed to handle {input_type} request")?;
                        if let Some(reply_to) = message.reply {{
                            let mut buf = ::bytes::BytesMut::with_capacity(reply.encoded_len());
                            reply
                                .encode(&mut buf)
                                .expect("to encode without reaching capacity");
                            client
                                .publish(reply_to, buf.into())
                                .await
                                .context("failed to publish reply")?;
                        }} else {{
                            eprintln!("No reply subject found in message");
                        }}
                    }},
                "#
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"
        // TODO: Consider this as a trait implementation for types that implement the Server trait
        #[allow(dead_code)]
        pub async fn start_server<S>(
            server: S,
            client: async_nats::Client,
        ) -> ::anyhow::Result<impl ::futures::Future<Output = ::anyhow::Result<()>>>
        where
            S: {name}Server + Send + 'static,
        {{
            let subject_prefix = server.subject_prefix().trim_end_matches('.');
            let mut subscription = client
                .subscribe(format!("{{subject_prefix}}.>"))
                .await
                .context("failed to subscribe for {name} messages")?;
            Ok(async move {{
                while let Some(message) = subscription.next().await {{
                    match message.subject.as_str().strip_prefix(&subject_prefix) {{
                        {matchy}
                        _ => {{
                            eprintln!("received message on unknown subject: {{}}", message.subject);
                        }}
                    }}
                }}
                Ok(())
            }})
        }}
        "#
    )
}

#[derive(PartialEq)]
enum MethodType {
    PubSub,
    RequestResponse,
}

fn get_method_type(method: &prost_build::Method) -> MethodType {
    /*
     * This still doesn't work. See https://github.com/tokio-rs/prost/pull/591
     * when merged, we'll be able to define based on options how to generate the code
     */

    // let options = &method.options;
    // let pubsub = options
    //     .iter()
    //     .find(|&option| option.identifier_value.clone().unwrap() == "pubsub");
    // match pubsub {
    //     Some(_) => MethodType::PubSub,
    //     None => MethodType::RequestResponse,
    // }

    // for now, let's get based on the name of the method
    // if starts with subscribe, it's pubsub; else, it's request/response
    let name = &method.name;
    if name.starts_with("subscribe") {
        MethodType::PubSub
    } else {
        MethodType::RequestResponse
    }
}

/// Convert a method name to a function name
fn convert_method_to_function(method: &str) -> String {
    method.to_case(Case::Snake)
}

// TODO: Consider validation, error handling, unit tests for desired results
/// Convert a method name to a NATS subject friendly name
fn convert_method_to_subject(method: &str) -> String {
    method.to_case(Case::Snake).replace("_", ".")
}

#[cfg(test)]
mod test {
    use crate::{convert_method_to_function, convert_method_to_subject};

    #[test]
    fn can_convert_to_function() {
        assert_eq!(
            convert_method_to_function("StartComponent"),
            "start_component"
        );
        assert_eq!(
            convert_method_to_function("StartProvider"),
            "start_provider"
        );
        assert_eq!(convert_method_to_function("PutConfig"), "put_config");
    }

    #[test]
    fn can_convert_to_subject() {
        assert_eq!(
            convert_method_to_subject("StartComponent"),
            "start.component"
        );
        assert_eq!(convert_method_to_subject("StartProvider"), "start.provider");
        assert_eq!(convert_method_to_subject("PutConfig"), "put.config");
    }
}
