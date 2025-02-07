mod generated {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
}

use futures::{stream, Stream, StreamExt};
use generated::*;

// Implement the service trait
struct PersonService;
impl PersonServiceServer for PersonService {
    async fn get_person(&self, _request: GetPersonRequest) -> anyhow::Result<GetPersonResponse> {
        Ok(GetPersonResponse {
            person: Some(Person {
                first_name: "John".to_string(),
                last_name: "Doe".to_string(),
                age: 42,
                address: Some(Address {
                    street: "1234 Elm St".to_string(),
                    city: "Springfield".to_string(),
                    state: "IL".to_string(),
                    zip_code: "62701".to_string(),
                }),
            }),
        })
    }

    async fn get_people(
        &self,
        _request: GetPersonRequest,
    ) -> anyhow::Result<impl Stream<Item = GetPersonResponse>> {
        let person = Person {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            age: 42,
            address: Some(Address {
                street: "1234 Elm St".to_string(),
                city: "Springfield".to_string(),
                state: "IL".to_string(),
                zip_code: "62701".to_string(),
            }),
        };
        let person2 = Person {
            first_name: "Bob".to_string(),
            ..person.clone()
        };
        let person3 = Person {
            first_name: "Alice".to_string(),
            ..person.clone()
        };
        Ok(stream::iter(vec![
            GetPersonResponse {
                person: Some(person),
            },
            GetPersonResponse {
                person: Some(person2),
            },
            GetPersonResponse {
                person: Some(person3),
            },
        ]))
    }
}

#[tokio::main]
async fn main() {
    // Connect to NATS
    let client = async_nats::connect("nats://127.0.0.1:4222")
        .await
        .expect("should be able to connect to NATS locally");

    // Start your service in a background task
    let service = PersonService {};
    let service_handle = tokio::spawn({
        let client = client.clone();
        start_server(service, client)
            .await
            .expect("should be able to start server")
    });

    // Make a request using the trait on the `async_nats::Client`
    let request = GetPersonRequest { id: 42 };
    let person = client
        .get_person(request)
        .await
        .expect("should be able to get person")
        .person
        .expect("person should be present");

    println!("Person: {person:?}");
    // Ensure the response is what we expect
    assert_eq!(person.first_name, "John");
    assert_eq!(person.last_name, "Doe");
    assert_eq!(person.age, 42);
    let address = person.address.expect("address should be present");
    assert_eq!(address.street, "1234 Elm St");
    assert_eq!(address.city, "Springfield");
    assert_eq!(address.state, "IL");
    assert_eq!(address.zip_code, "62701");

    // Putting imports here to show when you need them
    let mut people = client
        .get_people(request)
        .await
        .expect("should be able to get people");

    // NOTE: it's up to the user to close the stream when done. In this case we know
    // there is only one person in the stream so we break after the first one.
    let mut seen_john = false;
    let mut seen_bob = false;
    let mut seen_alice = false;
    while let Some(person) = people.next().await {
        let person = person.person.expect("person should be present");
        println!("Person from stream: {person:?}");
        // Ensure the response is what we expect
        match &*person.first_name {
            "John" => {
                seen_john = true;
            }
            "Bob" => {
                seen_bob = true;
            }
            "Alice" => {
                seen_alice = true;
            }
            _ => panic!("unexpected person"),
        }
        assert_eq!(person.last_name, "Doe");
        assert_eq!(person.age, 42);
        let address = person.address.expect("address should be present");
        assert_eq!(address.street, "1234 Elm St");
        assert_eq!(address.city, "Springfield");
        assert_eq!(address.state, "IL");
        assert_eq!(address.zip_code, "62701");
        if seen_john && seen_bob && seen_alice {
            break;
        }
    }

    // Should be no more items in the stream
    let remaining_person =
        tokio::time::timeout(std::time::Duration::from_millis(100), people.next()).await;
    assert!(remaining_person.is_err());

    service_handle.abort();
}
