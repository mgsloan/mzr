use yansi::Paint;

pub fn color_dir<T>(x: &T) -> Paint<&T> {
    Paint::blue(x).bold()
}

pub fn color_file<T>(x: &T) -> Paint<&T> {
    // TODO(cleanup): different color than color_dir?
    Paint::blue(x).bold()
}

pub fn color_zone_pid<T>(x: &T) -> Paint<&T> {
    // TODO(cleanup): different color than color_zone_name?
    Paint::yellow(x).bold()
}

pub fn color_zone_name<T>(x: &T) -> Paint<&T> {
    Paint::yellow(x).bold()
}

pub fn color_snap_name<T>(x: &T) -> Paint<&T> {
    Paint::cyan(x).bold()
}

pub fn color_err<T>(x: &T) -> Paint<&T> {
    Paint::red(x).bold()
}

pub fn color_warn<T>(x: &T) -> Paint<&T> {
    Paint::yellow(x).bold()
}

pub fn color_cmd<T>(x: &T) -> Paint<&T> {
    Paint::purple(x).bold()
}
