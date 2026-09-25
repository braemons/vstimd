//! Diagnostic: list every mode evdi's connector reports, in order, with
//! size/refresh/preferred-flag, to debug why the "first" mode picked isn't
//! always the display's native resolution.

use drm::control::Device as CtrlDevice;
use drm::control::ModeTypeFlags;

use vstimd::render::evdi::find_connected_evdi;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let node = find_connected_evdi().expect("no connected evdi node");
    let conn = node.card.get_connector(node.connector, false).expect("get_connector");

    println!("{} modes reported:", conn.modes().len());
    for (i, m) in conn.modes().iter().enumerate() {
        let (w, h) = m.size();
        let preferred = m.mode_type().contains(ModeTypeFlags::PREFERRED);
        println!(
            "  [{i}] {:?} {}x{} vrefresh={} clock={} preferred={}",
            m.name(),
            w,
            h,
            m.vrefresh(),
            m.clock(),
            preferred
        );
    }
}
