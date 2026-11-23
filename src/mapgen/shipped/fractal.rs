//! `CvFractal`: the height field every stock map script cuts its world from.
//!
//! Civilization VI's map scripts are Lua, but the field they sample is built
//! inside the engine: `Fractal.Create`, `Fractal.CreateRifts`,
//! `fractal:BuildRidges` and `fractal:GetHeight`. The scripts call them with
//! the argument lists Civilization V's scripts used, and Civilization V's
//! implementation is public (`CvFractal.cpp` in the game core DLL source), so
//! this is that implementation, statement for statement, in integers.
//!
//! Integer arithmetic is the point as much as the fidelity: a field built
//! with no float in it is the same field on every build target.

use crate::rng::Rng;

pub const FRAC_WRAP_X: u32 = 0x01;
pub const FRAC_WRAP_Y: u32 = 0x02;
pub const FRAC_POLAR: u32 = 0x08;
pub const FRAC_CENTER_RIFT: u32 = 0x10;
pub const FRAC_INVERT_HEIGHTS: u32 = 0x20;

/// `DEFAULT_FRAC_X_EXP` and `DEFAULT_FRAC_Y_EXP`, which a script asks for by
/// passing `-1`.
pub const DEFAULT_FRAC_X_EXP: i32 = 7;
pub const DEFAULT_FRAC_Y_EXP: i32 = 6;

const FLOAT_PRECISION: i32 = 1000;

/// `hexspaceDirections`: north-east, east, south-east, south-west, west,
/// north-west, as cube-coordinate steps.
const HEXSPACE_DIRECTIONS: [[i32; 3]; 6] = [
    [0, 1, -1],
    [1, 0, -1],
    [1, -1, 0],
    [0, -1, 1],
    [-1, 0, 1],
    [-1, 1, 0],
];

#[derive(Clone)]
pub struct CvFractal {
    /// `m_aaiFrac[x][y]`, `(frac_x + 1)` columns of `(frac_y + 1)` cells.
    frac: Vec<i32>,
    frac_x: i32,
    frac_y: i32,
    xs: i32,
    ys: i32,
    flags: u32,
    x_inc: i32,
    y_inc: i32,
}

struct VoronoiSeed {
    hex_x: i32,
    hex_y: i32,
    weakness: i32,
    bias_direction: i32,
    bias_strength: i32,
}

/// `xToHexspaceX`: odd rows sit half a hex east.
fn x_to_hexspace_x(x: i32, y: i32) -> i32 {
    x - if y >= 0 { y >> 1 } else { (y - 1) / 2 }
}

/// `hexDistance`, over a hexspace difference.
fn hex_distance(dx: i32, dy: i32) -> i32 {
    if (dx >= 0) == (dy >= 0) {
        dx.abs() + dy.abs()
    } else {
        dx.abs().max(dy.abs())
    }
}

/// Which of the six hex directions a hexspace step points along most nearly.
fn estimate_direction(dx: i32, dy: i32) -> Option<i32> {
    if dx == 0 && dy == 0 {
        return None;
    }
    let dz = -dx - dy;
    let mut best = 0;
    let mut best_index = None;
    for (index, direction) in HEXSPACE_DIRECTIONS.iter().enumerate() {
        let dot = dx * direction[0] + dy * direction[1] + dz * direction[2];
        if dot > best {
            best = dot;
            best_index = Some(index as i32);
        }
    }
    best_index
}

impl CvFractal {
    /// `Fractal.Create(iW, iH, grain, flags, fracXExp, fracYExp)`.
    pub fn create(
        rng: &mut Rng,
        xs: i32,
        ys: i32,
        grain: i32,
        flags: u32,
        frac_x_exp: i32,
        frac_y_exp: i32,
    ) -> Self {
        Self::init(rng, xs, ys, grain, flags, None, frac_x_exp, frac_y_exp)
    }

    /// `Fractal.CreateRifts(iW, iH, grain, flags, riftsFrac, fracXExp,
    /// fracYExp)`: the same field with a rift worn into it along the line the
    /// second field draws.
    #[allow(clippy::too_many_arguments)]
    pub fn create_rifts(
        rng: &mut Rng,
        xs: i32,
        ys: i32,
        grain: i32,
        flags: u32,
        rifts: &CvFractal,
        frac_x_exp: i32,
        frac_y_exp: i32,
    ) -> Self {
        Self::init(
            rng,
            xs,
            ys,
            grain,
            flags,
            Some(rifts),
            frac_x_exp,
            frac_y_exp,
        )
    }

    fn at(&self, x: i32, y: i32) -> i32 {
        self.frac[(x * (self.frac_y + 1) + y) as usize]
    }

    fn at_mut(&mut self, x: i32, y: i32) -> &mut i32 {
        let index = (x * (self.frac_y + 1) + y) as usize;
        &mut self.frac[index]
    }

    /// `CvFractal::fracInitInternal`, without the hints nothing passes.
    #[allow(clippy::too_many_arguments)]
    fn init(
        rng: &mut Rng,
        xs: i32,
        ys: i32,
        grain: i32,
        flags: u32,
        rifts: Option<&CvFractal>,
        frac_x_exp: i32,
        frac_y_exp: i32,
    ) -> Self {
        let frac_x_exp = if frac_x_exp < 0 {
            DEFAULT_FRAC_X_EXP
        } else {
            frac_x_exp
        };
        let frac_y_exp = if frac_y_exp < 0 {
            DEFAULT_FRAC_Y_EXP
        } else {
            frac_y_exp
        };
        let frac_x = 1 << frac_x_exp;
        let frac_y = 1 << frac_y_exp;
        let mut fractal = CvFractal {
            frac: vec![0; ((frac_x + 1) * (frac_y + 1)) as usize],
            frac_x,
            frac_y,
            xs,
            ys,
            flags,
            x_inc: (frac_x * FLOAT_PRECISION) / xs.max(1),
            y_inc: (frac_y * FLOAT_PRECISION) / ys.max(1),
        };
        let wrap_x = flags & FRAC_WRAP_X != 0;
        let wrap_y = flags & FRAC_WRAP_Y != 0;
        let polar = flags & FRAC_POLAR != 0;

        let min_exp = frac_x_exp.min(frac_y_exp);
        let smooth = (min_exp - grain).clamp(0, min_exp);

        for pass in (0..=smooth).rev() {
            let mut screen = 0;
            for bit in 0..=pass {
                screen |= 1 << bit;
            }

            if wrap_y {
                for x in 0..=frac_x {
                    *fractal.at_mut(x, frac_y) = fractal.at(x, 0);
                }
            } else if polar {
                for x in 0..=frac_x {
                    *fractal.at_mut(x, 0) = 0;
                    *fractal.at_mut(x, frac_y) = 0;
                }
            }

            if wrap_x {
                for y in 0..=frac_y {
                    *fractal.at_mut(frac_x, y) = fractal.at(0, y);
                }
            } else if polar {
                for y in 0..=frac_y {
                    *fractal.at_mut(0, y) = 0;
                    *fractal.at_mut(frac_x, y) = 0;
                }
            }

            if flags & FRAC_CENTER_RIFT != 0 {
                if wrap_y {
                    for x in 0..=frac_x {
                        for y in 0..(frac_y / 6) {
                            let divisor = ((frac_y / 12) - y).abs() + 1;
                            *fractal.at_mut(x, y) /= divisor;
                            *fractal.at_mut(x, (frac_y / 2) + y) /= divisor;
                        }
                    }
                }
                if wrap_x {
                    for y in 0..=frac_y {
                        for x in 0..(frac_x / 6) {
                            let divisor = ((frac_x / 12) - x).abs() + 1;
                            *fractal.at_mut(x, y) /= divisor;
                            *fractal.at_mut((frac_x / 2) + x, y) /= divisor;
                        }
                    }
                }
            }

            let columns = (frac_x >> pass) + i32::from(!wrap_x);
            let rows = (frac_y >> pass) + i32::from(!wrap_y);
            let noise = 1usize << (8 - smooth + pass);
            let half = 1 << (7 - smooth + pass);
            for x in 0..columns {
                for y in 0..rows {
                    let (cx, cy) = (x << pass, y << pass);
                    if pass == smooth {
                        *fractal.at_mut(cx, cy) = rng.below(256) as i32;
                        continue;
                    }
                    let sum = if cx & screen != 0 {
                        if cy & screen != 0 {
                            (fractal.at((x - 1) << pass, (y - 1) << pass)
                                + fractal.at((x + 1) << pass, (y - 1) << pass)
                                + fractal.at((x - 1) << pass, (y + 1) << pass)
                                + fractal.at((x + 1) << pass, (y + 1) << pass))
                                >> 2
                        } else {
                            (fractal.at((x - 1) << pass, cy) + fractal.at((x + 1) << pass, cy)) >> 1
                        }
                    } else if cy & screen != 0 {
                        (fractal.at(cx, (y - 1) << pass) + fractal.at(cx, (y + 1) << pass)) >> 1
                    } else {
                        // A corner, set on an earlier pass.
                        continue;
                    };
                    let jittered = sum + rng.below(noise) as i32 - half;
                    *fractal.at_mut(cx, cy) = jittered.clamp(0, 255);
                }
            }
        }

        if let Some(rifts) = rifts {
            fractal.tectonic_action(rifts);
        }

        if flags & FRAC_INVERT_HEIGHTS != 0 {
            for x in 0..frac_x {
                for y in 0..frac_y {
                    *fractal.at_mut(x, y) = 255 - fractal.at(x, y);
                }
            }
        }
        fractal
    }

    /// `CvFractal::getHeight`: the field under a map plot, bilinear over the
    /// four lattice points round it.
    pub fn height(&self, x: i32, y: i32) -> i32 {
        if x < 0 || x >= self.xs || y < 0 || y >= self.ys {
            return 0;
        }
        let low_x = ((self.x_inc * x) / FLOAT_PRECISION).min(self.frac_x - 1);
        let low_y = ((self.y_inc * y) / FLOAT_PRECISION).min(self.frac_y - 1);
        let err_x = (self.x_inc * x) - (low_x * FLOAT_PRECISION);
        let err_y = (self.y_inc * y) - (low_y * FLOAT_PRECISION);

        // The shipped sum is a 32-bit `int` and stays inside one: each weight
        // pair multiplies to at most a million and a height to at most 255.
        let mut sum = 0i64;
        sum += i64::from(FLOAT_PRECISION - err_x)
            * i64::from(FLOAT_PRECISION - err_y)
            * i64::from(self.at(low_x, low_y));
        sum += i64::from(err_x)
            * i64::from(FLOAT_PRECISION - err_y)
            * i64::from(self.at(low_x + 1, low_y));
        sum += i64::from(FLOAT_PRECISION - err_x)
            * i64::from(err_y)
            * i64::from(self.at(low_x, low_y + 1));
        sum += i64::from(err_x) * i64::from(err_y) * i64::from(self.at(low_x + 1, low_y + 1));
        sum /= i64::from(FLOAT_PRECISION) * i64::from(FLOAT_PRECISION);
        sum.clamp(0, 255) as i32
    }

    /// `CvFractal::getHeightFromPercent`, which a script reaches as
    /// `fractal:GetHeight(percent)`: the height that share of the lattice lies
    /// under, found by the engine's own bisection.
    pub fn height_from_percent(&self, percent: i32) -> i32 {
        let percent = percent.clamp(0, 100);
        let cells = self.frac_x * self.frac_y;
        let mut lower = 0;
        let mut upper = 255;
        let mut estimate = 255 * percent / 100;
        while estimate != lower {
            let mut below = 0;
            for x in 0..self.frac_x {
                for y in 0..self.frac_y {
                    if self.at(x, y) < estimate {
                        below += 1;
                    }
                }
            }
            if (100 * below) / cells > percent {
                upper = estimate;
            } else {
                lower = estimate;
            }
            estimate = (upper + lower) / 2;
        }
        estimate
    }

    /// `CvFractal::tectonicAction`: wear a rift down the map's east-west seam,
    /// wandering where the second field says.
    fn tectonic_action(&mut self, rifts: &CvFractal) {
        let rift_x = (self.frac_x / 4) * 3;
        let width = 16;
        for y in 0..=self.frac_y {
            for x in 0..width {
                let deep = 0;
                let centre = (((rifts.at(rift_x.min(rifts.frac_x), y.min(rifts.frac_y)) - 128)
                    * self.frac_x)
                    / 128)
                    / 8;
                let right = self.yield_x(centre + x);
                let left = self.yield_x(centre - x);
                *self.at_mut(right, y) = ((self.at(right, y) * x) + deep * (width - x)) / width;
                *self.at_mut(left, y) = ((self.at(left, y) * x) + deep * (width - x)) / width;
            }
        }
        for y in 0..=self.frac_y {
            *self.at_mut(self.frac_x, y) = self.at(0, y);
        }
    }

    fn yield_x(&self, bad_x: i32) -> i32 {
        if bad_x < 0 {
            bad_x + self.frac_x
        } else if bad_x >= self.frac_x {
            bad_x - self.frac_x
        } else {
            bad_x
        }
    }

    /// `fractal:BuildRidges(numPlates, flags, blendRidge, blendFract)`, which
    /// is `CvFractal::ridgeBuilder`: scatter plate seeds, and lift every cell
    /// by how nearly it stands on the line between its two nearest. A cell at
    /// a seed is lifted nothing and a cell on a boundary the whole 255, so the
    /// field's high ground comes to follow the seams between plates.
    ///
    /// A script that passes flags also gets plates of unequal strength, each
    /// leaning along one direction, and a boundary roughened cell by cell.
    pub fn build_ridges(
        &mut self,
        rng: &mut Rng,
        num_seeds: i32,
        ridge_flags: u32,
        blend_ridge: i32,
        blend_fract: i32,
    ) {
        let num_seeds = num_seeds.max(3) as usize;
        let mut seeds: Vec<VoronoiSeed> = Vec::with_capacity(num_seeds);
        for _ in 0..num_seeds {
            let hex_y = rng.below(self.frac_y as usize) as i32;
            let hex_x = x_to_hexspace_x(rng.below(self.frac_x as usize) as i32, hex_y);
            let weakness = (rng.below(7) as i32 - 3).max(0);
            let bias_direction = rng.below(6) as i32;
            let bias_strength = (rng.below(8) as i32 - 4).max(0);
            let mut seed = VoronoiSeed {
                hex_x,
                hex_y,
                weakness,
                bias_direction,
                bias_strength,
            };
            // A lattice too small to hold this many seeds seven apart would
            // spin here for ever; the stock sizes never come near it, and a
            // board under them gives up on the spacing instead.
            let mut redraws = 0;
            while redraws < 10_000
                && seeds.iter().any(|other| {
                    hex_distance(seed.hex_x - other.hex_x, seed.hex_y - other.hex_y) < 7
                })
            {
                seed.hex_x = rng.below(self.frac_x as usize) as i32;
                seed.hex_y = rng.below(self.frac_y as usize) as i32;
                redraws += 1;
            }
            seeds.push(seed);
        }

        for x in 0..self.frac_x {
            for y in 0..self.frac_y {
                let this_hex_x = x_to_hexspace_x(x, y);
                let this_hex_y = y;
                let mut closest = i32::MAX;
                let mut next_closest = i32::MAX;
                for seed in &seeds {
                    let mut distance =
                        hex_distance(this_hex_x - seed.hex_x, this_hex_y - seed.hex_y);
                    if ridge_flags != 0 {
                        distance += seed.weakness;
                        distance += rng.below(3) as i32;
                        let direction =
                            estimate_direction(seed.hex_x - this_hex_x, seed.hex_y - this_hex_y);
                        if direction == Some(seed.bias_direction) {
                            distance -= seed.bias_strength;
                        } else if direction == Some((seed.bias_direction + 3) % 6) {
                            distance += seed.bias_strength;
                        }
                        distance = distance.max(1);
                    }
                    if distance < closest {
                        next_closest = closest;
                        closest = distance;
                    } else if distance < next_closest {
                        next_closest = distance;
                    }
                }
                let ridge_height = if next_closest != 0 {
                    ((255 * i64::from(closest)) / i64::from(next_closest)) as i32
                } else {
                    0
                };
                let blended = (ridge_height * blend_ridge + self.at(x, y) * blend_fract)
                    / (blend_ridge + blend_fract).max(1);
                *self.at_mut(x, y) = blended;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script cuts its world at a percentile, so the height it is handed
    /// has to leave about that share of the lattice under it.
    #[test]
    fn a_percentile_cut_leaves_that_share_of_the_field_below_it() {
        let mut rng = Rng::new(7);
        let field = CvFractal::create(&mut rng, 74, 46, 2, FRAC_POLAR, 6, 5);
        for percent in [25, 50, 62, 80] {
            let height = field.height_from_percent(percent);
            let below = (0..field.frac_x)
                .flat_map(|x| (0..field.frac_y).map(move |y| (x, y)))
                .filter(|(x, y)| field.at(*x, *y) < height)
                .count() as i32;
            let share = 100 * below / (field.frac_x * field.frac_y);
            assert!(
                (share - percent).abs() <= 3,
                "cut at {percent}% left {share}% below it"
            );
        }
    }

    /// `FRAC_POLAR` without a wrap pins all four edges of the lattice to zero
    /// at the head of every pass, which is what holds a script's land off the
    /// rim of its map. The last pass still jitters the points it fills in, so
    /// an edge ends within that pass's noise of zero rather than on it.
    #[test]
    fn a_polar_field_is_held_down_along_every_edge() {
        let mut rng = Rng::new(3);
        let field = CvFractal::create(&mut rng, 74, 46, 2, FRAC_POLAR, 6, 5);
        // Grain 2 on a 2^5 lattice smooths over three passes; the last one
        // adds at most 2^(7 - 3) to a point between two zeros.
        let noise = 16;
        for x in 0..=field.frac_x {
            assert!(field.at(x, 0) <= noise);
            assert!(field.at(x, field.frac_y) <= noise);
        }
        for y in 0..=field.frac_y {
            assert!(field.at(0, y) <= noise);
            assert!(field.at(field.frac_x, y) <= noise);
        }
        let interior = field.height_from_percent(50);
        assert!(
            interior > 2 * noise,
            "the middle of the field stands at {interior}"
        );
    }

    /// A wrapping field has to meet itself across the seam, or every desert
    /// and range would break there. The engine copies the first column over
    /// the last at the head of each pass, so the two agree on every point the
    /// last pass left alone, which is every even row.
    #[test]
    fn a_wrapping_field_meets_itself_across_the_seam() {
        let mut rng = Rng::new(11);
        let field = CvFractal::create(&mut rng, 84, 54, 3, FRAC_WRAP_X, -1, -1);
        for y in (0..=field.frac_y).step_by(2) {
            assert_eq!(field.at(0, y), field.at(field.frac_x, y));
        }
    }

    /// A rift is worn to nothing along its own line: the seam column of a
    /// rifted field is all zeros where the plain field is not.
    #[test]
    fn a_rift_wears_the_field_to_nothing_along_its_line() {
        let mut rng = Rng::new(5);
        let rifts = CvFractal::create(&mut rng, 74, 46, 1, 0, 6, 5);
        let field = CvFractal::create_rifts(&mut rng, 74, 46, 2, FRAC_POLAR, &rifts, 6, 5);
        for y in 0..=field.frac_y {
            let centre = (((rifts.at((field.frac_x / 4) * 3, y) - 128) * field.frac_x) / 128) / 8;
            assert_eq!(field.at(field.yield_x(centre), y), 0);
        }
    }

    /// Ridges lift the lines between plates and leave the plates' own
    /// middles down, so a ridged field has its seeds in its low ground.
    #[test]
    fn ridges_follow_the_lines_between_plates() {
        let mut rng = Rng::new(9);
        let mut field = CvFractal::create(&mut rng, 84, 54, 3, FRAC_WRAP_X, -1, -1);
        for cell in &mut field.frac {
            *cell = 0;
        }
        field.build_ridges(&mut rng, 9, 0, 1, 0);
        let zeros = field.frac.iter().filter(|height| **height == 0).count();
        let peaks = field.frac.iter().filter(|height| **height == 255).count();
        assert!(zeros >= 9, "every seed stands at nothing, got {zeros}");
        assert!(peaks > 50, "the seams stand at the top, got {peaks}");
    }

    /// The same seed is the same field: nothing here reads a float, a hash
    /// order or the clock.
    #[test]
    fn the_same_seed_builds_the_same_field() {
        let build = || {
            let mut rng = Rng::new(21);
            let mut field = CvFractal::create(&mut rng, 74, 46, 3, FRAC_WRAP_X, -1, -1);
            field.build_ridges(&mut rng, 18, FRAC_WRAP_X, 10, 5);
            field.frac
        };
        assert_eq!(build(), build());
    }
}
