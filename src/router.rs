use super::{HttpCode, Method, Request, Response};

type Handler = fn(Request) -> Response;

#[derive(Default, Clone)]
pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn add_route(&mut self, route: Route) {
        self.routes.push(route);
    }

    pub fn route(&self, req: Request) -> Response {
        let mut response = Response::from(HttpCode::NotFound);

        if let Some(route) = self.routes.iter().find(|route| route.matches(&req)) {
            response = (route.handler)(req);
        }

        response
    }
}

#[derive(Clone)]
pub struct Route {
    path: String,
    handler: Box<Handler>,
    compare_path: ComparePath,
    methods: Vec<Method>,
}

impl Route {
    fn matches(&self, req: &Request) -> bool {
        let path_bool = match self.compare_path {
            ComparePath::Exact => self.path == req.path(),
            ComparePath::Prefix => req.path().starts_with(&self.path),
        };
        path_bool && self.methods.contains(&req.method())
    }

    pub fn get<S>(path: S, handler: Handler, compare_path: ComparePath) -> Self
    where
        S: Into<String>,
    {
        Route {
            path: path.into(),
            handler: Box::new(handler),
            compare_path,
            methods: vec![Method::Get],
        }
    }

    pub fn post<S>(path: S, handler: Handler, compare_path: ComparePath) -> Self
    where
        S: Into<String>,
    {
        Route {
            path: path.into(),
            handler: Box::new(handler),
            compare_path,
            methods: vec![Method::Post],
        }
    }
}

#[derive(Clone, Copy)]
pub enum ComparePath {
    Exact,
    Prefix,
}
