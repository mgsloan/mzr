use yansi::Paint;

pub fn color_dir<T>(x: &T) -> Paint<&T> {
    Paint::blue(x)
}

pub fn color_zone_name<T>(x: &T) -> Paint<&T> {
    Paint::red(x)
}

pub fn color_err<T>(x: &T) -> Paint<&T> {
    Paint::red(x)
}
