# m5-solar: where the sun is

Milestone M5 (Sun). Kernel crate `modeler-kernel`, module `src/sun.rs` (skeleton on main).

## Goal
Given a place on Earth (latitude, longitude, the model's north angle, the clock's offset from UTC) and a local date and time, compute where the sun is in the sky and the unit vector toward it in model axes. The renderer will light the model and cast shadows from that vector (later tasks).

The calendar helpers (`days_in_month`, `LocalTime::new`, `is_valid`, `day_of_year`, `from_day_of_year`) are already implemented on main with their tests (`tests/sun_calendar.rs`); leave them as they are.

## What to implement
Fill in the `unimplemented!("m5-solar")` bodies in `src/sun.rs`:

- `julian_day(year, month, day, hours)`: the Julian day number of a Gregorian calendar date plus `hours / 24` (hours may be fractional, negative or above 24). Use the standard conversion that treats January and February as months 13 and 14 of the previous year and applies the Gregorian century correction. 2000-01-01 12:00 is 2451545.0.
- `solar_coordinates(jd)`: the sun's apparent declination, apparent right ascension (degrees, 0..360) and the equation of time (minutes, apparent minus mean solar time) at Julian day `jd`. Use the well-known low-precision solar theory: Julian centuries `T` since J2000.0; the sun's geometric mean longitude and mean anomaly as polynomials in `T`; Earth's orbital eccentricity; the equation of center (three sine terms of the mean anomaly); the apparent longitude corrected for nutation and aberration with the longitude of the moon's ascending node; the mean obliquity of the ecliptic corrected the same way. The equation of time follows from the mean longitude, mean anomaly, eccentricity and `tan²(ε/2)`. This is the method behind common public-domain solar calculators; accuracy of about 0.01 degree over 1900-2100 is expected. Don't model ΔT (treat UTC as dynamical time).
- `solar_position_at(latitude, longitude, jd)`: azimuth (degrees clockwise from true north, in `[0, 360)`) and apparent elevation of the sun seen from that place at Julian day `jd` (UTC). True solar time = minutes since UTC midnight + equation of time + 4 × longitude; hour angle = true solar time / 4 − 180 degrees; zenith from latitude, declination and hour angle; azimuth from the same (measure from north, clockwise). Add atmospheric refraction to the elevation using the usual piecewise approximation for standard pressure and temperature (zero above 85 degrees; about 0.5 to 0.6 degree at the horizon; small negative-elevation branch below −0.575 degree). Clamp the cosine of the zenith to `[-1, 1]` before `acos`.
- `solar_position(geo, time)`: the same for a `LocalTime` on the clock of `geo`: UTC hours = `hour + minute / 60 − utc_offset`, Julian day from `julian_day(year, month, day, utc_hours)`, then `solar_position_at(geo.latitude, geo.longitude, jd)`. It must give exactly the same result as calling `solar_position_at` with that Julian day.
- `direction_from(position, north_angle)`: the unit vector toward the sun in model axes. Model axes: +X red, +Y green, +Z up. True north is +Y turned clockwise (seen from +Z) by `north_angle` degrees, so with azimuth `A` and elevation `e` the direction is `(sin(A + n)·cos e, cos(A + n)·cos e, sin e)`.
- `sun_direction` is already written on top of the two above.

## Rules
- No new dependencies. Pure functions, no panics for finite inputs (any latitude in −90..=90, longitude −180..=180, offset −12..=14, years 1900..2100 at least).
- Write the code from the description above; don't copy code from other projects.
- Don't change the public signatures or the types in `src/sun.rs`, and don't touch `ShadowSettings`/`Geolocation` defaults.

## Acceptance
`cargo test -p modeler-kernel --test sun` (published reference values for the Julian day, solar coordinates and a full solar position case; solstice, equinox, polar and hemisphere checks; refraction size; direction axes and north angle; property tests for ranges, unit length, north angle as a turn about Z, and same-local-solar-time invariance). Plus the usual gates: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, all workspace tests.
