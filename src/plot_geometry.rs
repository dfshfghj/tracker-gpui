//! Full-resolution display geometry and the library's tessellated path cache.
use crate::plot::{PlotPoint, PlotSeries};
use gpui_component::plot::PathCache;
use gpui_kit::{Path, Pixels, Point};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub x_range: (f64, f64),
    pub y_range: (f64, f64),
}
impl Viewport {
    pub fn project(self, point: &PlotPoint) -> (f32, f32) {
        let x = ((point.x - self.x_range.0) / (self.x_range.1 - self.x_range.0).max(1e-12)) as f32
            * self.width;
        let y = (1. - (point.y - self.y_range.0) / (self.y_range.1 - self.y_range.0).max(1e-12))
            as f32
            * self.height;
        (x.clamp(0., self.width), y.clamp(0., self.height))
    }
}
pub(crate) struct Geometry {
    pub positions: Vec<(f32, f32)>,
    path: RefCell<PathCache>,
}
impl Geometry {
    pub fn line_path(&self, origin: Point<Pixels>) -> Option<Path<Pixels>> {
        // Each Geometry owns one immutable series/viewport, so this key stays
        // fixed. PathCache translates cached vertices without tessellating again.
        self.path.borrow_mut().get(0, origin, || {
            crate::canvas::stroke_path(&self.positions, 1.5)
        })
    }
}
#[derive(Default)]
pub(crate) struct GeometryCache(Option<CachedGeometry>);
struct CachedGeometry {
    series: Rc<PlotSeries>,
    viewport: Viewport,
    geometry: Rc<Geometry>,
}
impl GeometryCache {
    pub fn get(&mut self, series: &Rc<PlotSeries>, viewport: Viewport) -> Rc<Geometry> {
        if let Some(cache) = &self.0 {
            if Rc::ptr_eq(&cache.series, series) && cache.viewport == viewport {
                return cache.geometry.clone();
            }
        }
        let geometry = Rc::new(Geometry {
            positions: series.points.iter().map(|p| viewport.project(p)).collect(),
            path: RefCell::new(PathCache::default()),
        });
        self.0 = Some(CachedGeometry {
            series: series.clone(),
            viewport,
            geometry: geometry.clone(),
        });
        geometry
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use gpui_kit::{point, px};
    fn series() -> Rc<PlotSeries> {
        Rc::new(PlotSeries {
            points: (0..100_000)
                .map(|frame| PlotPoint {
                    frame,
                    x: frame as f64,
                    y: ((frame * 7919 % 1000) as f64 / 500.) - 1.,
                })
                .collect(),
            x_range: (0., 100_000.),
            y_range: (-1., 1.),
        })
    }
    fn viewport() -> Viewport {
        Viewport {
            width: 500.,
            height: 100.,
            scale: 1.,
            x_range: (0., 100_000.),
            y_range: (-1., 1.),
        }
    }
    #[test]
    fn full_geometry_keeps_every_point_in_order_including_overlapping_points() {
        let data = series();
        let view = viewport();
        let mut cache = GeometryCache::default();
        let geometry = cache.get(&data, view);
        assert_eq!(geometry.positions.len(), 100_000);
        for (p, q) in data.points.iter().zip(&geometry.positions) {
            assert_eq!(view.project(p), *q);
        }
        let repeated = Rc::new(PlotSeries {
            points: vec![data.points[0]; 10],
            x_range: data.x_range,
            y_range: data.y_range,
        });
        assert_eq!(cache.get(&repeated, view).positions.len(), 10);
    }
    #[test]
    fn geometry_reuses_on_scroll_and_updates_for_resize_dpi_range_and_data() {
        let data = series();
        let view = viewport();
        let mut cache = GeometryCache::default();
        let original = cache.get(&data, view);
        assert!(Rc::ptr_eq(&original, &cache.get(&data, view)));
        for changed in [
            Viewport {
                width: 1000.,
                ..view
            },
            Viewport { scale: 2., ..view },
            Viewport {
                x_range: (0., 1000.),
                ..view
            },
        ] {
            assert!(!Rc::ptr_eq(&original, &cache.get(&data, changed)));
        }
        let changed = Rc::new(PlotSeries {
            points: vec![],
            x_range: data.x_range,
            y_range: data.y_range,
        });
        assert!(cache.get(&changed, view).positions.is_empty());
        assert_eq!(data.points.len(), 100_000);
    }
    #[test]
    fn tessellated_path_survives_origin_changes() {
        let data = Rc::new(PlotSeries {
            points: vec![
                PlotPoint {
                    frame: 0,
                    x: 0.,
                    y: 0.,
                },
                PlotPoint {
                    frame: 1,
                    x: 1.,
                    y: 1.,
                },
            ],
            x_range: (0., 1.),
            y_range: (0., 1.),
        });
        let mut cache = GeometryCache::default();
        let geometry = cache.get(
            &data,
            Viewport {
                x_range: data.x_range,
                y_range: data.y_range,
                ..viewport()
            },
        );
        let a = geometry.line_path(point(px(0.), px(0.))).unwrap();
        assert!(geometry.path.borrow().is_warm());
        let b = geometry.line_path(point(px(20.), px(30.))).unwrap();
        assert_eq!(a.vertices.len(), b.vertices.len());
        for (a, b) in a.vertices.iter().zip(&b.vertices) {
            let delta = b.xy_position - a.xy_position;
            assert!((f32::from(delta.x) - 20.).abs() < 1e-4);
            assert!((f32::from(delta.y) - 30.).abs() < 1e-4);
        }
    }
}
