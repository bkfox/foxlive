/// Generic Service trait that handles requests and call corresponding RPC method.
pub trait Service {
    type Request;
    type Response;
    // type ResponseFut;

    /// Process a request returning the called method's response
    fn process_request(&mut self, request: Self::Request) -> Option<Self::Response>;

    // Process a request returning a future resolving to method's response
    // fn process(&mut self, request: Self::Request) -> Self::ResponseFut;
}


#[cfg(test)]
mod test {
    // TODO: client impl & futures using a channel

    use crate as libfoxlive;
    use libfoxlive_derive::*;
    use super::*;

    struct SimpleService {
        a: u32,
    }

    #[service]
    impl SimpleService {
        fn add(&mut self, b: u32) -> u32 {
            self.a += b;
            self.a
        }

        fn sub(&mut self, c: u32) -> u32 {
            self.a -= c;
            self.a
        }
    }

    #[test]
    fn test_service() {
        let mut service = SimpleService { a: 0 };
        match service.process_request(service::Request::Add(13)) {
            Some(service::Response::Add(13)) => {},
            _ => panic!("invalid response for `Add()`"),
        };

        match service.process_request(service::Request::Sub(1)) {
            Some(service::Response::Sub(12)) => {},
            _ => panic!("invalid response for `Sub()`"),
        };
    }


}



