//! 本地 OCR：ONNX Runtime 推理、行检测识别、版面分区、段落拼接、评测。
//!
//! M4 开始实现。

/// 轴对齐外框，单位为物理像素。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BBox {
    /// 两个外框是否相交（边缘相接不算）。
    pub fn intersects(&self, o: &BBox) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_intersection() {
        let a = BBox { x: 0.0, y: 0.0, w: 10.0, h: 10.0 };
        let b = BBox { x: 5.0, y: 5.0, w: 10.0, h: 10.0 };
        let c = BBox { x: 10.0, y: 0.0, w: 5.0, h: 5.0 };
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }
}
