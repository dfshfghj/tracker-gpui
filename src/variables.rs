//! Derived data share one parser, dimension checker and evaluator across plots and tables.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{coords::ImageCoordSystem, plot::PlotVar, track::Track};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    pub id: u64,
    pub name: String,
    pub expression: String,
    #[serde(default = "visible")]
    pub table_visible: bool,
}

fn visible() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dimension([f64; 3]); // length, time, angle (degrees)

impl Dimension {
    fn add(self, other: Self, sign: f64) -> Self {
        Self(std::array::from_fn(|i| self.0[i] + sign * other.0[i]))
    }
    fn scale(self, power: f64) -> Self {
        Self(self.0.map(|x| x * power))
    }
    fn same(self, other: Self) -> bool {
        self.0
            .iter()
            .zip(other.0)
            .all(|(a, b)| (a - b).abs() < 1e-9)
    }
    pub fn unit(self, length: &str) -> String {
        let factor = |symbol: &str, power: f64| {
            let symbol = if symbol.contains(['/', '*', '·', '^', ' ']) {
                format!("({symbol})")
            } else {
                symbol.to_string()
            };
            if (power - 1.).abs() < 1e-9 {
                symbol
            } else if (power - 2.).abs() < 1e-9 {
                format!("{symbol}²")
            } else {
                format!("{symbol}^{power}")
            }
        };
        let mut top = Vec::new();
        let mut bottom = Vec::new();
        for (symbol, power) in [length, "s", "°"].into_iter().zip(self.0) {
            if power > 1e-9 {
                top.push(factor(symbol, power));
            }
            if power < -1e-9 {
                bottom.push(factor(symbol, -power));
            }
        }
        let numerator = top.join("·");
        if bottom.is_empty() {
            numerator
        } else {
            let numerator = if numerator.is_empty() {
                "1".to_string()
            } else {
                numerator
            };
            let denominator = bottom.join("·");
            if bottom.len() > 1 {
                format!("{numerator}/({denominator})")
            } else {
                format!("{numerator}/{denominator}")
            }
        }
    }
}

fn error(key: &str, detail: impl std::fmt::Display) -> String {
    rust_i18n::t!(key, detail = detail.to_string()).to_string()
}

pub const BUILTINS: &[(&str, PlotVar)] = &[
    ("frame", PlotVar::Frame),
    ("t", PlotVar::Time),
    ("x", PlotVar::X),
    ("y", PlotVar::Y),
    ("vx", PlotVar::Vx),
    ("vy", PlotVar::Vy),
    ("ax", PlotVar::Ax),
    ("ay", PlotVar::Ay),
    ("speed", PlotVar::Speed),
    ("length", PlotVar::Length),
    ("angle", PlotVar::Angle),
    ("arm1", PlotVar::Arm1),
    ("arm2", PlotVar::Arm2),
    ("radius", PlotVar::Radius),
    ("center_x", PlotVar::CenterX),
    ("center_y", PlotVar::CenterY),
];

fn dimension(var: PlotVar) -> Dimension {
    use PlotVar::*;
    Dimension(match var {
        Time => [0., 1., 0.],
        X | Y | Length | Arm1 | Arm2 | Radius | CenterX | CenterY => [1., 0., 0.],
        Vx | Vy | Speed => [1., -1., 0.],
        Ax | Ay => [1., -2., 0.],
        Angle => [0., 0., 1.],
        _ => [0.; 3],
    })
}

#[derive(Clone, Debug)]
enum Raw {
    Number(f64),
    Name(String),
    Unary(Box<Raw>),
    Binary(u8, Box<Raw>, Box<Raw>),
    Call(String, Vec<Raw>),
}

struct Parser<'a> {
    text: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn parse(text: &'a str) -> Result<Raw, String> {
        if text.len() > 2048 {
            return Err(error("variables.error.limit", 2048));
        }
        let mut p = Self {
            text: text.as_bytes(),
            pos: 0,
            depth: 0,
        };
        let expr = p.expr(0)?;
        p.space();
        if p.pos != p.text.len() {
            return p.fail();
        }
        Ok(expr)
    }
    fn fail<T>(&self) -> Result<T, String> {
        Err(error("variables.error.syntax", self.pos + 1))
    }
    fn space(&mut self) {
        while self.text.get(self.pos).is_some_and(u8::is_ascii_whitespace) {
            self.pos += 1;
        }
    }
    fn take(&mut self, c: u8) -> bool {
        self.space();
        if self.text.get(self.pos) == Some(&c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self, min: u8) -> Result<Raw, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err(error("variables.error.limit", 64));
        }
        self.space();
        let mut left = if self.take(b'-') {
            Raw::Unary(Box::new(self.expr(3)?))
        } else if self.take(b'+') {
            self.expr(3)?
        } else if self.take(b'(') {
            let e = self.expr(0)?;
            if !self.take(b')') {
                return self.fail();
            }
            e
        } else if self
            .text
            .get(self.pos)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            let start = self.pos;
            while self
                .text
                .get(self.pos)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                self.pos += 1;
            }
            if self
                .text
                .get(self.pos)
                .is_some_and(|c| *c == b'e' || *c == b'E')
            {
                self.pos += 1;
                if self
                    .text
                    .get(self.pos)
                    .is_some_and(|c| *c == b'+' || *c == b'-')
                {
                    self.pos += 1;
                }
                while self.text.get(self.pos).is_some_and(u8::is_ascii_digit) {
                    self.pos += 1;
                }
            }
            let value = std::str::from_utf8(&self.text[start..self.pos])
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .ok_or_else(|| error("variables.error.syntax", start + 1))?;
            Raw::Number(value)
        } else {
            let start = self.pos;
            while self
                .text
                .get(self.pos)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
            {
                self.pos += 1;
            }
            if start == self.pos {
                return self.fail();
            }
            let name = std::str::from_utf8(&self.text[start..self.pos])
                .unwrap()
                .to_string();
            if self.take(b'(') {
                let mut args = Vec::new();
                if !self.take(b')') {
                    loop {
                        args.push(self.expr(0)?);
                        if self.take(b')') {
                            break;
                        }
                        if !self.take(b',') || args.len() >= 2 {
                            return self.fail();
                        }
                    }
                }
                Raw::Call(name, args)
            } else {
                Raw::Name(name)
            }
        };
        loop {
            self.space();
            let Some(&op) = self.text.get(self.pos) else {
                break;
            };
            let (l, r) = match op {
                b'+' | b'-' => (1, 2),
                b'*' | b'/' => (2, 3),
                b'^' => (4, 4),
                _ => break,
            };
            if l < min {
                break;
            }
            self.pos += 1;
            left = Raw::Binary(op, Box::new(left), Box::new(self.expr(r)?));
        }
        self.depth -= 1;
        Ok(left)
    }
}

#[derive(Clone, Debug)]
struct Expr {
    kind: Kind,
    dim: Dimension,
}
#[derive(Clone, Debug)]
enum Kind {
    Number(f64),
    Var(PlotVar),
    Unary(Box<Expr>),
    Binary(u8, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

impl Expr {
    fn value(&self, lookup: &mut impl FnMut(PlotVar) -> Option<f64>) -> Option<f64> {
        let value = match &self.kind {
            Kind::Number(v) => *v,
            Kind::Var(var) => lookup(*var)?,
            Kind::Unary(e) => -e.value(lookup)?,
            Kind::Binary(op, a, b) => {
                let a = a.value(lookup)?;
                let b = b.value(lookup)?;
                match op {
                    b'+' => a + b,
                    b'-' => a - b,
                    b'*' => a * b,
                    b'/' => a / b,
                    _ => a.powf(b),
                }
            }
            Kind::Call(name, args) => {
                let a = args[0].value(lookup)?;
                let trig = if args[0].dim.same(Dimension([0., 0., 1.])) {
                    a.to_radians()
                } else {
                    a
                };
                match name.as_str() {
                    "sqrt" => a.sqrt(),
                    "abs" => a.abs(),
                    "sin" => trig.sin(),
                    "cos" => trig.cos(),
                    "tan" => trig.tan(),
                    "asin" => a.asin().to_degrees(),
                    "acos" => a.acos().to_degrees(),
                    "atan" => a.atan().to_degrees(),
                    "exp" => a.exp(),
                    "ln" => a.ln(),
                    "log10" => a.log10(),
                    "min" => a.min(args[1].value(lookup)?),
                    "max" => a.max(args[1].value(lookup)?),
                    "hypot" => a.hypot(args[1].value(lookup)?),
                    _ => return None,
                }
            }
        };
        value.is_finite().then_some(value)
    }
}

pub struct Variables {
    expressions: BTreeMap<u64, Expr>,
}

impl Variables {
    pub fn compile(track: &Track) -> Result<Self, String> {
        if track.variables.len() > 64 {
            return Err(error("variables.error.limit", 64));
        }
        let mut names = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for def in &track.variables {
            if !valid_name(&def.name) {
                return Err(error("variables.error.name", &def.name));
            }
            if !names.insert(&def.name) || !ids.insert(def.id) {
                return Err(error("variables.error.duplicate", &def.name));
            }
        }
        let raw: BTreeMap<_, _> = track
            .variables
            .iter()
            .map(|d| Ok((d.id, Parser::parse(&d.expression)?)))
            .collect::<Result<_, String>>()?;
        let mut compiler = Compiler {
            track,
            raw,
            done: BTreeMap::new(),
            visiting: BTreeSet::new(),
        };
        for def in &track.variables {
            compiler.variable(def.id)?;
        }
        Ok(Self {
            expressions: compiler.done,
        })
    }
    pub fn unit(&self, id: u64, length: &str) -> Option<String> {
        self.expressions.get(&id).map(|e| e.dim.unit(length))
    }
    pub fn axis_label(&self, var: PlotVar, track: &Track, length: &str) -> String {
        if let PlotVar::Custom(id) = var {
            let name = track
                .variables
                .iter()
                .find(|d| d.id == id)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| var.label());
            let unit = self.unit(id, length).unwrap_or_else(|| "?".into());
            if unit.is_empty() {
                name
            } else {
                format!("{name} ({unit})")
            }
        } else {
            var.axis_label(length)
        }
    }
    pub fn value(
        &self,
        var: PlotVar,
        track: &Track,
        frame: usize,
        coords: &ImageCoordSystem,
        times: &[f64],
    ) -> Option<f64> {
        fn evaluate(
            vars: &Variables,
            var: PlotVar,
            track: &Track,
            frame: usize,
            coords: &ImageCoordSystem,
            times: &[f64],
            cache: &mut BTreeMap<u64, Option<f64>>,
        ) -> Option<f64> {
            if let PlotVar::Custom(id) = var {
                if let Some(value) = cache.get(&id) {
                    return *value;
                }
                let value = vars
                    .expressions
                    .get(&id)?
                    .value(&mut |v| evaluate(vars, v, track, frame, coords, times, cache));
                cache.insert(id, value);
                value
            } else {
                var.value(track, frame, coords, times)
                    .filter(|v| v.is_finite())
            }
        }
        evaluate(self, var, track, frame, coords, times, &mut BTreeMap::new())
    }
}

struct Compiler<'a> {
    track: &'a Track,
    raw: BTreeMap<u64, Raw>,
    done: BTreeMap<u64, Expr>,
    visiting: BTreeSet<u64>,
}

impl Compiler<'_> {
    fn variable(&mut self, id: u64) -> Result<Dimension, String> {
        if let Some(expr) = self.done.get(&id) {
            return Ok(expr.dim);
        }
        let name = &self
            .track
            .variables
            .iter()
            .find(|v| v.id == id)
            .unwrap()
            .name;
        if !self.visiting.insert(id) {
            return Err(error("variables.error.cycle", name));
        }
        let expr = self.expression(&self.raw[&id].clone())?;
        let dim = expr.dim;
        self.done.insert(id, expr);
        self.visiting.remove(&id);
        Ok(dim)
    }
    fn expression(&mut self, raw: &Raw) -> Result<Expr, String> {
        let zero = Dimension::default();
        let (kind, dim) = match raw {
            Raw::Number(v) => (Kind::Number(*v), zero),
            Raw::Name(name) => {
                if name == "pi" {
                    return Ok(Expr {
                        kind: Kind::Number(std::f64::consts::PI),
                        dim: zero,
                    });
                }
                if name == "e" {
                    return Ok(Expr {
                        kind: Kind::Number(std::f64::consts::E),
                        dim: zero,
                    });
                }
                let var = if let Some((_, var)) = BUILTINS.iter().find(|(s, _)| *s == name) {
                    if !var.available(self.track) {
                        return Err(error("variables.error.unknown", name));
                    }
                    *var
                } else {
                    PlotVar::Custom(
                        self.track
                            .variables
                            .iter()
                            .find(|d| d.name == *name)
                            .ok_or_else(|| error("variables.error.unknown", name))?
                            .id,
                    )
                };
                let dim = if let PlotVar::Custom(id) = var {
                    self.variable(id)?
                } else {
                    dimension(var)
                };
                (Kind::Var(var), dim)
            }
            Raw::Unary(e) => {
                let e = self.expression(e)?;
                let dim = e.dim;
                (Kind::Unary(Box::new(e)), dim)
            }
            Raw::Binary(op, a, b) => {
                let a = self.expression(a)?;
                let b = self.expression(b)?;
                let dim = match op {
                    b'+' | b'-' => {
                        if !a.dim.same(b.dim) {
                            return Err(error("variables.error.dimension", *op as char));
                        }
                        a.dim
                    }
                    b'*' => a.dim.add(b.dim, 1.),
                    b'/' => a.dim.add(b.dim, -1.),
                    _ => {
                        if !b.dim.same(zero) {
                            return Err(error("variables.error.dimension", "^"));
                        }
                        if a.dim.same(zero) {
                            zero
                        } else {
                            let power = b
                                .value(&mut |_| None)
                                .filter(|p| p.abs() <= 32.)
                                .ok_or_else(|| error("variables.error.power", "^"))?;
                            a.dim.scale(power)
                        }
                    }
                };
                if dim.0.iter().any(|v| !v.is_finite() || v.abs() > 64.) {
                    return Err(error("variables.error.limit", 64));
                }
                (Kind::Binary(*op, Box::new(a), Box::new(b)), dim)
            }
            Raw::Call(name, raw_args) => {
                let count = match name.as_str() {
                    "min" | "max" | "hypot" => 2,
                    "sqrt" | "abs" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp"
                    | "ln" | "log10" => 1,
                    _ => return Err(error("variables.error.function", name)),
                };
                if raw_args.len() != count {
                    return Err(error("variables.error.function", name));
                }
                let args: Vec<Expr> = raw_args
                    .iter()
                    .map(|e| self.expression(e))
                    .collect::<Result<_, _>>()?;
                let dim = match name.as_str() {
                    "sqrt" => args[0].dim.scale(0.5),
                    "abs" => args[0].dim,
                    "min" | "max" | "hypot" => {
                        if !args[0].dim.same(args[1].dim) {
                            return Err(error("variables.error.dimension", name));
                        }
                        args[0].dim
                    }
                    "sin" | "cos" | "tan" => {
                        if !args[0].dim.same(zero) && !args[0].dim.same(Dimension([0., 0., 1.])) {
                            return Err(error("variables.error.dimension", name));
                        }
                        zero
                    }
                    _ => {
                        if !args[0].dim.same(zero) {
                            return Err(error("variables.error.dimension", name));
                        }
                        if matches!(name.as_str(), "asin" | "acos" | "atan") {
                            Dimension([0., 0., 1.])
                        } else {
                            zero
                        }
                    }
                };
                (Kind::Call(name.clone(), args), dim)
            }
        };
        Ok(Expr { kind, dim })
    }
}

pub fn valid_name(name: &str) -> bool {
    name.len() <= 48
        && name
            .as_bytes()
            .first()
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        && !BUILTINS.iter().any(|(s, _)| *s == name)
        && ![
            "pi", "e", "sqrt", "abs", "sin", "cos", "tan", "asin", "acos", "atan", "exp", "ln",
            "log10", "min", "max", "hypot",
        ]
        .contains(&name)
}

impl Track {
    pub fn set_variable(
        &mut self,
        id: Option<u64>,
        name: &str,
        expression: &str,
    ) -> Result<u64, String> {
        let mut candidate = self.clone();
        let id = id.unwrap_or_else(|| {
            self.variables
                .iter()
                .map(|v| v.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
        });
        if let Some(def) = candidate.variables.iter_mut().find(|v| v.id == id) {
            // Names are stable references; rename dependents token by token in the same transaction.
            let old = def.name.clone();
            def.name = name.trim().into();
            def.expression = expression.trim().into();
            for other in &mut candidate.variables {
                if other.id != id {
                    other.expression = rename(&other.expression, &old, name.trim());
                }
            }
        } else {
            candidate.variables.push(Definition {
                id,
                name: name.trim().into(),
                expression: expression.trim().into(),
                table_visible: true,
            });
        }
        Variables::compile(&candidate)?;
        self.variables = candidate.variables;
        Ok(id)
    }

    pub fn remove_variable(&mut self, id: u64) -> Result<(), String> {
        let mut candidate = self.clone();
        candidate.variables.retain(|v| v.id != id);
        Variables::compile(&candidate)?;
        self.variables = candidate.variables;
        Ok(())
    }
}

fn rename(text: &str, old: &str, new: &str) -> String {
    // Split numeric tokens too: never rename the exponent in a literal such as 1e3.
    let mut result = String::new();
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.') {
            let word = &text[start..index];
            result.push_str(if word == old { new } else { word });
            result.push(ch);
            start = index + ch.len_utf8();
        }
    }
    let word = &text[start..];
    result.push_str(if word == old { new } else { word });
    result
}

#[cfg(test)]
mod tests {
    use super::{Definition, Variables};
    use crate::{
        coords::ImageCoordSystem,
        plot::{self, PlotConfig, PlotVar},
        track::Track,
    };

    fn track() -> Track {
        let mut t = Track::point_mass(1, "test", 0);
        t.set_point(0, 0., 0.);
        t.set_point(1, 3., 4.);
        t.set_point(2, 6., 8.);
        t
    }

    #[test]
    fn dimensions_and_values_share_the_plot_data_pipeline() {
        let mut track = track();
        let r = track.set_variable(None, "r", "sqrt(x^2 + y^2)").unwrap();
        let q = track.set_variable(None, "q", "0.5 * speed^2").unwrap();
        let v = Variables::compile(&track).unwrap();
        assert_eq!(v.unit(r, "m").unwrap(), "m");
        assert_eq!(v.unit(q, "cm").unwrap(), "cm²/s²");
        let coords = ImageCoordSystem::default();
        let times = [0., 1., 2.];
        assert_eq!(
            v.value(PlotVar::Custom(r), &track, 1, &coords, &times),
            Some(5.)
        );
        let config = PlotConfig {
            x_var: PlotVar::Time,
            y_var: PlotVar::Custom(r),
            ..PlotConfig::default()
        };
        let series = plot::series(&track, &config, &coords, &times);
        assert_eq!(series.points.len(), 3);
        assert_eq!(series.points[1].y, 5.);
        assert_eq!(config.y_var.axis_label_for(&track, "m"), "r (m)");
        assert!(PlotVar::all_for(&track).contains(&PlotVar::Custom(r)));
    }

    #[test]
    fn changes_are_transactional_and_dependent_names_follow_renames() {
        let mut track = track();
        let r = track.set_variable(None, "r", "hypot(x,y)").unwrap();
        let rr = track.set_variable(None, "rr", "r^2").unwrap();
        let sum = track.set_variable(None, "sum", "r + x").unwrap();
        let before = track.clone();
        assert!(track.set_variable(Some(r), "r", "t").is_err());
        assert_eq!(track, before);
        assert!(track.remove_variable(r).is_err());
        assert_eq!(track, before);
        track
            .set_variable(Some(r), "distance", "hypot(x,y)")
            .unwrap();
        assert_eq!(
            track
                .variables
                .iter()
                .find(|v| v.id == rr)
                .unwrap()
                .expression,
            "distance^2"
        );
        let vars = Variables::compile(&track).unwrap();
        assert_eq!(vars.unit(rr, "m").unwrap(), "m²");
        track.remove_variable(sum).unwrap();
        track.remove_variable(rr).unwrap();
        track.remove_variable(r).unwrap();
        assert!(track.variables.is_empty());
    }

    #[test]
    fn bad_units_names_and_cycles_are_rejected() {
        let mut track = track();
        for expr in [
            "x+t",
            "sin(x)",
            "ln(t)",
            "hypot(x,t)",
            "x^t",
            "x^(x/x)",
            "unknown+1",
            "sqrt()",
            "min(x)",
            "x;",
        ] {
            assert!(track.set_variable(None, "bad", expr).is_err(), "{expr}");
        }
        assert!(track.set_variable(None, "x", "1").is_err());
        assert!(track.set_variable(None, "2a", "1").is_err());
        let a = track.set_variable(None, "a", "x").unwrap();
        track.set_variable(None, "b", "a*2").unwrap();
        assert!(track.set_variable(Some(a), "a", "b").is_err());
        track.variables = vec![Definition {
            id: 1,
            name: "looped".into(),
            expression: "looped+1".into(),
            table_visible: true,
        }];
        assert!(Variables::compile(&track).is_err());
    }

    #[test]
    fn precedence_fractional_units_and_invalid_sample_values() {
        let mut track = track();
        let tests = [
            ("power", "2^3^2", Some(512.)),
            ("negative", "-2^2", Some(-4.)),
            ("root", "sqrt(-1)", None),
            ("division", "1/0", None),
            ("log", "ln(-1)", None),
        ];
        for (name, expression, expected) in tests {
            let id = track.set_variable(None, name, expression).unwrap();
            assert_eq!(
                Variables::compile(&track).unwrap().value(
                    PlotVar::Custom(id),
                    &track,
                    1,
                    &ImageCoordSystem::default(),
                    &[0., 1., 2.]
                ),
                expected
            );
        }
        let root = track.set_variable(None, "root_length", "sqrt(x)").unwrap();
        let inverse = track.set_variable(None, "frequency", "1/t").unwrap();
        let vars = Variables::compile(&track).unwrap();
        assert_eq!(vars.unit(root, "m").unwrap(), "m^0.5");
        assert_eq!(vars.unit(inverse, "m").unwrap(), "1/s");
    }

    #[test]
    fn angle_functions_convert_degrees_and_dimensionless_inputs_use_radians() {
        let mut track = track();
        let a = track.set_variable(None, "alpha", "asin(1)").unwrap();
        let s = track.set_variable(None, "sine", "sin(alpha)").unwrap();
        let c = track.set_variable(None, "cosine", "cos(pi)").unwrap();
        let vars = Variables::compile(&track).unwrap();
        assert_eq!(vars.unit(a, "m").unwrap(), "°");
        assert_eq!(vars.unit(s, "m").unwrap(), "");
        assert_eq!(
            vars.value(
                PlotVar::Custom(s),
                &track,
                0,
                &ImageCoordSystem::default(),
                &[]
            ),
            Some(1.)
        );
        assert_eq!(
            vars.value(
                PlotVar::Custom(c),
                &track,
                0,
                &ImageCoordSystem::default(),
                &[]
            ),
            Some(-1.)
        );
    }

    #[test]
    fn project_restore_and_undo_keep_definitions_and_axis_references() {
        let text = r#"{
            "path":"sample.mp4", "tracks":[{
                "name":"test", "points":[{"frame":0,"x":3,"y":4}],
                "variables":[{"id":7,"name":"r","expression":"hypot(x,y)","table_visible":false}]
            }], "plot":{"configs":[{"x_var":"Time","y_var":{"Custom":7}}]}
        }"#;
        let project: crate::project::DocProject = serde_json::from_str(text).unwrap();
        let encoded = serde_json::to_string(&project).unwrap();
        let decoded: crate::project::DocProject = serde_json::from_str(&encoded).unwrap();
        let mut doc = crate::DocState::placeholder();
        doc.restore_project(&decoded);
        assert_eq!(doc.tracks[0].variables[0].id, 7);
        assert!(!doc.tracks[0].variables[0].table_visible);
        assert_eq!(doc.plot.configs[0].y_var, PlotVar::Custom(7));
        assert_eq!(
            PlotVar::Custom(7).value(&doc.tracks[0], 0, &doc.coords, &[]),
            Some(5.)
        );
        let before = doc.snapshot();
        let mut history = crate::history::UndoStack::new(before);
        doc.tracks[0]
            .set_variable(Some(7), "radius_value", "hypot(x,y)*2")
            .unwrap();
        history.record(doc.snapshot());
        doc.restore_snapshot(&history.undo().unwrap());
        assert_eq!(doc.tracks[0].variables[0].name, "r");
        doc.restore_snapshot(&history.redo().unwrap());
        assert_eq!(doc.tracks[0].variables[0].name, "radius_value");
        assert_eq!(
            PlotVar::Custom(7).value(&doc.tracks[0], 0, &doc.coords, &[]),
            Some(10.)
        );
    }
}
