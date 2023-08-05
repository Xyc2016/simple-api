use crate::types::ViewPathArgs;
use crate::view::View;
use regex::Regex;
use std::sync::Arc;

fn caps_to_map(re: &Regex, caps: &regex::Captures) -> ViewPathArgs {
    re.capture_names()
        .flatten()
        .filter_map(|n| Some((n.to_owned(), caps.name(n)?.as_str().to_owned())))
        .collect()
}

pub fn match_view(
    routes: &Vec<Arc<dyn View>>,
    path: &str,
) -> Option<(Arc<dyn View>, ViewPathArgs)> {
    for view in routes.iter() {
        let re = &view.re_path();
        if let Some(caps) = re.captures(&path) {
            let view_args = caps_to_map(re, &caps);
            return Some((view.clone(), view_args));
        }
    }
    None
}
