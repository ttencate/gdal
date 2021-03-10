use crate::spatial_ref::{CoordTransform, SpatialRef};
use crate::utils::{_last_null_pointer_err, _string};
use gdal_sys::{self, OGRErr, OGRGeometryH, OGRwkbGeometryType};
use libc::{c_double, c_int, c_void};
use std::cell::RefCell;
use std::ffi::CString;
use std::fmt::{self, Debug};
use std::ptr::null_mut;

use crate::errors::*;

/// OGR Geometry
pub struct Geometry {
    c_geometry_ref: RefCell<Option<OGRGeometryH>>,
    owned: bool,
}

impl Geometry {
    /// Create a new Geometry without a C pointer
    ///
    /// # Safety
    /// This method returns a Geometry without wrapped pointer
    pub unsafe fn lazy_feature_geometry() -> Geometry {
        // Geometry objects created with this method map to a Feature's
        // geometry whose memory is managed by the GDAL feature.
        // This object has a tricky lifecycle:
        //
        // * Initially it's created with a null c_geometry
        // * The first time `Feature::geometry` is called, it gets
        //   c_geometry from GDAL and calls `set_c_geometry` with it.
        // * When the Feature is destroyed, this object is also destroyed,
        //   which is good, because that's when c_geometry (which is managed
        //   by the GDAL feature) becomes invalid. Because `self.owned` is
        //   `true`, we don't call `OGR_G_DestroyGeometry`.
        Geometry {
            c_geometry_ref: RefCell::new(None),
            owned: false,
        }
    }

    pub fn has_gdal_ptr(&self) -> bool {
        self.c_geometry_ref.borrow().is_some()
    }

    /// Set the wrapped C pointer
    ///
    /// # Safety
    /// This method operates on a raw C pointer    
    pub unsafe fn set_c_geometry(&self, c_geometry: OGRGeometryH) {
        assert!(!self.has_gdal_ptr());
        assert_eq!(self.owned, false);
        *(self.c_geometry_ref.borrow_mut()) = Some(c_geometry);
    }

    pub(crate) unsafe fn with_c_geometry(c_geom: OGRGeometryH, owned: bool) -> Geometry {
        Geometry {
            c_geometry_ref: RefCell::new(Some(c_geom)),
            owned,
        }
    }

    pub fn empty(wkb_type: OGRwkbGeometryType::Type) -> Result<Geometry> {
        let c_geom = unsafe { gdal_sys::OGR_G_CreateGeometry(wkb_type) };
        if c_geom.is_null() {
            return Err(_last_null_pointer_err("OGR_G_CreateGeometry"));
        };
        Ok(unsafe { Geometry::with_c_geometry(c_geom, true) })
    }

    pub fn is_empty(&self) -> bool {
        unsafe { gdal_sys::OGR_G_IsEmpty(self.c_geometry()) == 1 }
    }

    /// Create a geometry by parsing a
    /// [WKT](https://en.wikipedia.org/wiki/Well-known_text) string.
    pub fn from_wkt(wkt: &str) -> Result<Geometry> {
        let c_wkt = CString::new(wkt)?;
        let c_wkt_ptr = c_wkt.into_raw();
        let mut c_wkt_end_ptr = c_wkt_ptr; // The pointer is updated to point just beyond the consumed bytes.
        let mut c_geom = null_mut();
        let rv = unsafe { gdal_sys::OGR_G_CreateFromWkt(&mut c_wkt_end_ptr, null_mut(), &mut c_geom) };
        unsafe { CString::from_raw(c_wkt_ptr); }
        if rv != OGRErr::OGRERR_NONE {
            return Err(GdalError::OgrError {
                err: rv,
                method_name: "OGR_G_CreateFromWkt",
            });
        }
        Ok(unsafe { Geometry::with_c_geometry(c_geom, true) })
    }

    /// Create a rectangular geometry from West, South, East and North values.
    pub fn bbox(w: f64, s: f64, e: f64, n: f64) -> Result<Geometry> {
        Geometry::from_wkt(&format!(
            "POLYGON (({} {}, {} {}, {} {}, {} {}, {} {}))",
            w, n, e, n, e, s, w, s, w, n,
        ))
    }

    /// Serialize the geometry as JSON.
    pub fn json(&self) -> Result<String> {
        let c_json = unsafe { gdal_sys::OGR_G_ExportToJson(self.c_geometry()) };
        if c_json.is_null() {
            return Err(_last_null_pointer_err("OGR_G_ExportToJson"));
        };
        let rv = _string(c_json);
        unsafe { gdal_sys::VSIFree(c_json as *mut c_void) };
        Ok(rv)
    }

    /// Serialize the geometry as WKT.
    pub fn wkt(&self) -> Result<String> {
        let mut c_wkt = null_mut();
        let rv = unsafe { gdal_sys::OGR_G_ExportToWkt(self.c_geometry(), &mut c_wkt) };
        if rv != OGRErr::OGRERR_NONE {
            return Err(GdalError::OgrError {
                err: rv,
                method_name: "OGR_G_ExportToWkt",
            });
        }
        let wkt = _string(c_wkt);
        unsafe { gdal_sys::OGRFree(c_wkt as *mut c_void) };
        Ok(wkt)
    }

    /// Returns a C pointer to the wrapped Geometry
    ///
    /// # Safety
    /// This method returns a raw C pointer
    pub unsafe fn c_geometry(&self) -> OGRGeometryH {
        self.c_geometry_ref.borrow().unwrap()
    }

    /// Returns the C pointer of a Geometry
    ///
    /// # Safety
    /// This method returns a raw C pointer
    pub unsafe fn into_c_geometry(mut self) -> OGRGeometryH {
        assert!(self.owned);
        self.owned = false;
        self.c_geometry()
    }

    pub fn set_point(&mut self, i: usize, point: (f64, f64, f64)) {
        let (x, y, z) = point;
        unsafe {
            gdal_sys::OGR_G_SetPoint(
                self.c_geometry(),
                i as c_int,
                x as c_double,
                y as c_double,
                z as c_double,
            );
        };
    }

    pub fn set_point_2d(&mut self, i: usize, p: (f64, f64)) {
        let (x, y) = p;
        unsafe {
            gdal_sys::OGR_G_SetPoint_2D(self.c_geometry(), i as c_int, x as c_double, y as c_double)
        };
    }

    pub fn add_point(&mut self, p: (f64, f64, f64)) {
        let (x, y, z) = p;
        unsafe {
            gdal_sys::OGR_G_AddPoint(
                self.c_geometry(),
                x as c_double,
                y as c_double,
                z as c_double,
            )
        };
    }

    pub fn add_point_2d(&mut self, p: (f64, f64)) {
        let (x, y) = p;
        unsafe { gdal_sys::OGR_G_AddPoint_2D(self.c_geometry(), x as c_double, y as c_double) };
    }

    pub fn get_point(&self, i: i32) -> (f64, f64, f64) {
        let mut x: c_double = 0.;
        let mut y: c_double = 0.;
        let mut z: c_double = 0.;
        unsafe { gdal_sys::OGR_G_GetPoint(self.c_geometry(), i, &mut x, &mut y, &mut z) };
        (x as f64, y as f64, z as f64)
    }

    pub fn get_point_vec(&self) -> Vec<(f64, f64, f64)> {
        let length = unsafe { gdal_sys::OGR_G_GetPointCount(self.c_geometry()) };
        (0..length).map(|i| self.get_point(i)).collect()
    }

    /// Compute the convex hull of this geometry.
    pub fn convex_hull(&self) -> Result<Geometry> {
        let c_geom = unsafe { gdal_sys::OGR_G_ConvexHull(self.c_geometry()) };
        if c_geom.is_null() {
            return Err(_last_null_pointer_err("OGR_G_ConvexHull"));
        };
        Ok(unsafe { Geometry::with_c_geometry(c_geom, true) })
    }

    #[cfg(any(all(major_is_2, minor_ge_1), major_ge_3))]
    pub fn delaunay_triangulation(&self, tolerance: Option<f64>) -> Result<Self> {
        let c_geom = unsafe {
            gdal_sys::OGR_G_DelaunayTriangulation(self.c_geometry(), tolerance.unwrap_or(0.0), 0)
        };
        if c_geom.is_null() {
            return Err(_last_null_pointer_err("OGR_G_DelaunayTriangulation"));
        };

        Ok(unsafe { Geometry::with_c_geometry(c_geom, true) })
    }

    pub fn simplify(&self, tolerance: f64) -> Result<Self> {
        let c_geom = unsafe { gdal_sys::OGR_G_Simplify(self.c_geometry(), tolerance) };
        if c_geom.is_null() {
            return Err(_last_null_pointer_err("OGR_G_Simplify"));
        };

        Ok(unsafe { Geometry::with_c_geometry(c_geom, true) })
    }

    pub fn simplify_preserve_topology(&self, tolerance: f64) -> Result<Self> {
        let c_geom =
            unsafe { gdal_sys::OGR_G_SimplifyPreserveTopology(self.c_geometry(), tolerance) };
        if c_geom.is_null() {
            return Err(_last_null_pointer_err("OGR_G_SimplifyPreserveTopology"));
        };

        Ok(unsafe { Geometry::with_c_geometry(c_geom, true) })
    }

    pub fn geometry_type(&self) -> OGRwkbGeometryType::Type {
        unsafe { gdal_sys::OGR_G_GetGeometryType(self.c_geometry()) }
    }

    pub fn geometry_count(&self) -> usize {
        let cnt = unsafe { gdal_sys::OGR_G_GetGeometryCount(self.c_geometry()) };
        cnt as usize
    }

    /// Returns the n-th sub-geometry as a non-owned Geometry.
    ///
    /// # Safety
    /// Don't keep this object for long.
    pub unsafe fn get_unowned_geometry(&self, n: usize) -> Geometry {
        // get the n-th sub-geometry as a non-owned Geometry; don't keep this
        // object for long.
        let c_geom = gdal_sys::OGR_G_GetGeometryRef(self.c_geometry(), n as c_int);
        Geometry::with_c_geometry(c_geom, false)
    }

    pub fn add_geometry(&mut self, mut sub: Geometry) -> Result<()> {
        assert!(sub.owned);
        sub.owned = false;
        let rv =
            unsafe { gdal_sys::OGR_G_AddGeometryDirectly(self.c_geometry(), sub.c_geometry()) };
        if rv != OGRErr::OGRERR_NONE {
            return Err(GdalError::OgrError {
                err: rv,
                method_name: "OGR_G_AddGeometryDirectly",
            });
        }
        Ok(())
    }

    // Transform the geometry inplace (when we own the Geometry)
    pub fn transform_inplace(&mut self, htransform: &CoordTransform) -> Result<()> {
        let rv = unsafe { gdal_sys::OGR_G_Transform(self.c_geometry(), htransform.to_c_hct()) };
        if rv != OGRErr::OGRERR_NONE {
            return Err(GdalError::OgrError {
                err: rv,
                method_name: "OGR_G_Transform",
            });
        }
        Ok(())
    }

    // Return a new transformed geometry (when the Geometry is owned by a Feature)
    pub fn transform(&self, htransform: &CoordTransform) -> Result<Geometry> {
        let new_c_geom = unsafe { gdal_sys::OGR_G_Clone(self.c_geometry()) };
        let rv = unsafe { gdal_sys::OGR_G_Transform(new_c_geom, htransform.to_c_hct()) };
        if rv != OGRErr::OGRERR_NONE {
            return Err(GdalError::OgrError {
                err: rv,
                method_name: "OGR_G_Transform",
            });
        }
        Ok(unsafe { Geometry::with_c_geometry(new_c_geom, true) })
    }

    pub fn transform_to_inplace(&mut self, spatial_ref: &SpatialRef) -> Result<()> {
        let rv = unsafe { gdal_sys::OGR_G_TransformTo(self.c_geometry(), spatial_ref.to_c_hsrs()) };
        if rv != OGRErr::OGRERR_NONE {
            return Err(GdalError::OgrError {
                err: rv,
                method_name: "OGR_G_TransformTo",
            });
        }
        Ok(())
    }

    /// Transforms a geometrys coordinates into another SpatialRef
    pub fn transform_to(&self, spatial_ref: &SpatialRef) -> Result<Geometry> {
        let new_c_geom = unsafe { gdal_sys::OGR_G_Clone(self.c_geometry()) };
        let rv = unsafe { gdal_sys::OGR_G_TransformTo(new_c_geom, spatial_ref.to_c_hsrs()) };
        if rv != OGRErr::OGRERR_NONE {
            return Err(GdalError::OgrError {
                err: rv,
                method_name: "OGR_G_TransformTo",
            });
        }
        Ok(unsafe { Geometry::with_c_geometry(new_c_geom, true) })
    }

    pub fn area(&self) -> f64 {
        unsafe { gdal_sys::OGR_G_Area(self.c_geometry()) }
    }

    /// May or may not contain a reference to a SpatialRef: if not, it returns
    /// an `Ok(None)`; if it does, it tries to build a SpatialRef. If that
    /// succeeds, it returns an Ok(Some(SpatialRef)), otherwise, you get the
    /// Err.
    ///
    pub fn spatial_ref(&self) -> Option<SpatialRef> {
        let c_spatial_ref = unsafe { gdal_sys::OGR_G_GetSpatialReference(self.c_geometry()) };

        if c_spatial_ref.is_null() {
            None
        } else {
            match SpatialRef::from_c_obj(c_spatial_ref) {
                Ok(sr) => Some(sr),
                Err(_) => None,
            }
        }
    }

    pub fn set_spatial_ref(&mut self, spatial_ref: SpatialRef) {
        unsafe {
            gdal_sys::OGR_G_AssignSpatialReference(self.c_geometry(), spatial_ref.to_c_hsrs())
        };
    }
}

impl Drop for Geometry {
    fn drop(&mut self) {
        if self.owned {
            let c_geometry = self.c_geometry_ref.borrow();
            unsafe { gdal_sys::OGR_G_DestroyGeometry(c_geometry.unwrap()) };
        }
    }
}

impl Clone for Geometry {
    fn clone(&self) -> Geometry {
        // assert!(self.has_gdal_ptr());
        let c_geometry = self.c_geometry_ref.borrow();
        let new_c_geom = unsafe { gdal_sys::OGR_G_Clone(c_geometry.unwrap()) };
        unsafe { Geometry::with_c_geometry(new_c_geom, true) }
    }
}

impl Debug for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.wkt() {
            Ok(wkt) => f.write_str(wkt.as_str()),
            Err(_) => Err(fmt::Error),
        }
    }
}

impl PartialEq for Geometry {
    fn eq(&self, other: &Self) -> bool {
        unsafe { gdal_sys::OGR_G_Equal(self.c_geometry(), other.c_geometry()) != 0 }
    }
}

impl Eq for Geometry {}

#[cfg(test)]
mod tests {
    use super::Geometry;
    use crate::spatial_ref::SpatialRef;

    #[test]
    #[allow(clippy::float_cmp)]
    pub fn test_area() {
        let geom = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbMultiPolygon).unwrap();
        assert_eq!(geom.area(), 0.0);

        let geom = Geometry::from_wkt("POINT(0 0)").unwrap();
        assert_eq!(geom.area(), 0.0);

        let wkt = "POLYGON ((45.0 45.0, 45.0 50.0, 50.0 50.0, 50.0 45.0, 45.0 45.0))";
        let geom = Geometry::from_wkt(wkt).unwrap();
        assert_eq!(geom.area().floor(), 25.0);
    }

    #[test]
    pub fn test_is_empty() {
        let geom = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbMultiPolygon).unwrap();
        assert!(geom.is_empty());

        let geom = Geometry::from_wkt("POINT(0 0)").unwrap();
        assert!(!geom.is_empty());

        let wkt = "POLYGON ((45.0 45.0, 45.0 50.0, 50.0 50.0, 50.0 45.0, 45.0 45.0))";
        let geom = Geometry::from_wkt(wkt).unwrap();
        assert!(!geom.is_empty());
    }

    #[test]
    pub fn test_create_multipoint_2d() {
        let mut geom = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbMultiPoint).unwrap();
        let mut point = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbPoint).unwrap();
        point.add_point_2d((1.0, 2.0));
        geom.add_geometry(point).unwrap();
        let mut point = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbPoint).unwrap();
        point.add_point_2d((2.0, 3.0));
        assert!(!point.is_empty());
        point.set_point_2d(0, (2.0, 4.0));
        geom.add_geometry(point).unwrap();
        assert!(!geom.is_empty());

        let expected = Geometry::from_wkt("MULTIPOINT((1.0 2.0), (2.0 4.0))").unwrap();
        assert_eq!(geom, expected);
    }

    #[test]
    pub fn test_create_multipoint_3d() {
        let mut geom = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbMultiPoint).unwrap();
        let mut point = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbPoint).unwrap();
        point.add_point((1.0, 2.0, 3.0));
        geom.add_geometry(point).unwrap();
        let mut point = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbPoint).unwrap();
        point.add_point((3.0, 2.0, 1.0));
        assert!(!point.is_empty());
        point.set_point(0, (4.0, 2.0, 1.0));
        geom.add_geometry(point).unwrap();
        assert!(!geom.is_empty());

        let expected = Geometry::from_wkt("MULTIPOINT((1.0 2.0 3.0), (4.0 2.0 1.0))").unwrap();
        assert_eq!(geom, expected);
    }

    #[test]
    pub fn test_spatial_ref() {
        let geom = Geometry::empty(::gdal_sys::OGRwkbGeometryType::wkbMultiPolygon).unwrap();
        assert!(geom.spatial_ref().is_none());

        let geom = Geometry::from_wkt("POINT(0 0)").unwrap();
        assert!(geom.spatial_ref().is_none());

        let wkt = "POLYGON ((45.0 45.0, 45.0 50.0, 50.0 50.0, 50.0 45.0, 45.0 45.0))";
        let mut geom = Geometry::from_wkt(wkt).unwrap();
        assert!(geom.spatial_ref().is_none());

        let srs = SpatialRef::from_epsg(4326).unwrap();
        geom.set_spatial_ref(srs);
        assert!(geom.spatial_ref().is_some());
    }
}
