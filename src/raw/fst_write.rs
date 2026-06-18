//! A minimal write trait for FST construction that works under `alloc` (no std).
//!
//! Under `std`, the blanket impl covers all `io::Write` types.
//! Under `alloc`-only, dedicated impls for `Vec<u8>` and `&mut W` are provided.

use crate::error::Result;

/// Trait abstracting byte output for FST building.
///
/// This is intentionally minimal: only `write_all` and `flush` are required.
pub trait FstWrite {
    /// Write `buf` entirely, or return an error.
    fn fst_write_all(&mut self, buf: &[u8]) -> Result<()>;
    /// Flush buffered data (no-op for `Vec<u8>`).
    fn fst_flush(&mut self) -> Result<()>;
}

// ── alloc-only (no std): Vec<u8> and &mut W impls ─────────────────────────

#[cfg(all(feature = "alloc", not(feature = "std")))]
impl FstWrite for alloc::vec::Vec<u8> {
    #[inline]
    fn fst_write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }

    #[inline]
    fn fst_flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(all(feature = "alloc", not(feature = "std")))]
impl<W: FstWrite + ?Sized> FstWrite for &mut W {
    #[inline]
    fn fst_write_all(&mut self, buf: &[u8]) -> Result<()> {
        (**self).fst_write_all(buf)
    }

    #[inline]
    fn fst_flush(&mut self) -> Result<()> {
        (**self).fst_flush()
    }
}

// ── std: blanket impl for all io::Write types ─────────────────────────────

#[cfg(feature = "std")]
impl<W: std::io::Write> FstWrite for W {
    #[inline]
    fn fst_write_all(&mut self, buf: &[u8]) -> Result<()> {
        std::io::Write::write_all(self, buf)?;
        Ok(())
    }

    #[inline]
    fn fst_flush(&mut self) -> Result<()> {
        std::io::Write::flush(self)?;
        Ok(())
    }
}
