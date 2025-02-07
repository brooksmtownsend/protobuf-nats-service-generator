mod generated {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
}
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

    service_handle.abort();
}
