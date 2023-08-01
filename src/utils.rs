pub mod cookie {
    use std::collections::HashMap;
    use cookie::Cookie;
    use crate::types::CookieMap;

    pub fn parse_cookie(cookies_string: &str) -> CookieMap {
        let mut map = HashMap::new();
        for cookie in Cookie::split_parse(cookies_string).into_iter() {
            let cookie = match cookie {
                Ok(v) => v,
                Err(_) => continue,
            };
            map.insert(cookie.name().to_string(), cookie.value().to_string());
        }
        map
    }
}
