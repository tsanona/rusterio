#[ambassador::delegatable_trait_remote]
pub trait GeometryTrait {
    type T;
    type PointType<'a>: 'a + PointTrait<T = Self::T>
    where
        Self: 'a;
    type LineStringType<'a>: 'a + LineStringTrait<T = Self::T>
    where
        Self: 'a;
    type PolygonType<'a>: 'a + PolygonTrait<T = Self::T>
    where
        Self: 'a;
    type MultiPointType<'a>: 'a + MultiPointTrait<T = Self::T>
    where
        Self: 'a;
    type MultiLineStringType<'a>: 'a + MultiLineStringTrait<T = Self::T>
    where
        Self: 'a;
    type MultiPolygonType<'a>: 'a + MultiPolygonTrait<T = Self::T>
    where
        Self: 'a;
    type GeometryCollectionType<'a>: 'a + GeometryCollectionTrait<T = Self::T>
    where
        Self: 'a;
    type RectType<'a>: 'a + RectTrait<T = Self::T>
    where
        Self: 'a;
    type TriangleType<'a>: 'a + TriangleTrait<T = Self::T>
    where
        Self: 'a;
    type LineType<'a>: 'a + LineTrait<T = Self::T>
    where
        Self: 'a;
    fn dim(&self) -> geo_traits::Dimensions;
    fn as_type(
        &self,
    ) -> geo_traits::GeometryType<
        '_,
        Self::PointType<'_>,
        Self::LineStringType<'_>,
        Self::PolygonType<'_>,
        Self::MultiPointType<'_>,
        Self::MultiLineStringType<'_>,
        Self::MultiPolygonType<'_>,
        Self::GeometryCollectionType<'_>,
        Self::RectType<'_>,
        Self::TriangleType<'_>,
        Self::LineType<'_>,
    >;
}

#[ambassador::delegatable_trait_remote]
pub trait RectTrait {
    type CoordType<'a>: 'a + CoordTrait<T = <Self as crate::GeometryTrait>::T>
    where
        Self: 'a;
    fn min<'a>(&'a self) -> Self::CoordType<'a>;
    fn max<'a>(&'a self) -> Self::CoordType<'a>;
}
