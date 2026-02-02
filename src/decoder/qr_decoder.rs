use crate::decoder::bitstream::BitstreamExtractor;
use crate::decoder::format::FormatInfo;
use crate::decoder::function_mask::{alignment_pattern_positions, FunctionMask};
use crate::decoder::modes::{alphanumeric::AlphanumericDecoder, numeric::NumericDecoder};
use crate::decoder::reed_solomon::ReedSolomonDecoder;
use crate::decoder::tables::ec_block_info;
use crate::decoder::unmask::unmask;
use crate::decoder::version::VersionInfo;
/// Main QR code decoder - wires everything together
use crate::models::{BitMatrix, ECLevel, Point, QRCode, Version};
use crate::utils::geometry::PerspectiveTransform;

/// Main QR decoder that processes a detected QR region
pub struct QrDecoder;

impl QrDecoder {
    /// Decode a QR code from a binary matrix and finder pattern locations
    pub fn decode(
        matrix: &BitMatrix,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        module_size: f32,
    ) -> Option<QRCode> {
        #[cfg(debug_assertions)]
        eprintln!("    DECODE: module_size={:.2}", module_size);

        // Calculate the bottom-right corner
        let bottom_right = Self::calculate_bottom_right(top_left, top_right, bottom_left)?;
        #[cfg(debug_assertions)]
        eprintln!(
            "    DECODE: bottom_right=({:.1}, {:.1})",
            bottom_right.x, bottom_right.y
        );

        // Determine QR code dimension (version) estimate
        let estimated_dimension =
            Self::estimate_dimension(top_left, top_right, &bottom_right, module_size)?;
        #[cfg(debug_assertions)]
        eprintln!("    DECODE: estimated_dimension={}", estimated_dimension);

        let estimated_version = ((estimated_dimension - 17) / 4) as i32;
        let mut candidates = Vec::new();
        for delta in -2..=2 {
            let v = estimated_version + delta;
            if (1..=40).contains(&v) {
                candidates.push(v as u8);
            }
        }
        for v in 1..=40u8 {
            if !candidates.contains(&v) {
                candidates.push(v);
            }
        }

        for version_num in candidates {
            let dimension = 17 + 4 * version_num as usize;
            let transform = match Self::build_transform(
                top_left,
                top_right,
                bottom_left,
                &bottom_right,
                dimension,
            ) {
                Some(t) => t,
                None => continue,
            };
            let transform = Self::refine_transform_with_alignment(
                matrix,
                &transform,
                version_num,
                dimension,
                module_size,
                top_left,
                top_right,
                bottom_left,
            )
            .unwrap_or(transform);
            let qr_matrix = Self::extract_qr_region_with_transform(matrix, &transform, dimension);

            if let Some(qr) = Self::decode_from_matrix(&qr_matrix, version_num) {
                return Some(qr);
            }

            // Try inverted grid (binarization might be flipped)
            let inverted = Self::invert_matrix(&qr_matrix);
            if let Some(qr) = Self::decode_from_matrix(&inverted, version_num) {
                return Some(qr);
            }
        }

        None
    }

    /// Decode using grayscale sampling to build the QR matrix (more robust for real photos).
    pub fn decode_with_gray(
        binary: &BitMatrix,
        gray: &[u8],
        width: usize,
        height: usize,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        module_size: f32,
    ) -> Option<QRCode> {
        let bottom_right = Self::calculate_bottom_right(top_left, top_right, bottom_left)?;
        let mut br_candidates = Vec::new();
        let step = module_size.max(1.0) * 2.0;
        for dy in [-2.0f32, 0.0, 2.0] {
            for dx in [-2.0f32, 0.0, 2.0] {
                br_candidates.push(Point::new(
                    bottom_right.x + dx * step,
                    bottom_right.y + dy * step,
                ));
            }
        }
        let estimated_dimension =
            Self::estimate_dimension(top_left, top_right, &bottom_right, module_size)?;

        let estimated_version = ((estimated_dimension - 17) / 4) as i32;
        let mut candidates = Vec::new();
        for delta in -2..=2 {
            let v = estimated_version + delta;
            if (1..=40).contains(&v) {
                candidates.push(v as u8);
            }
        }
        for v in 1..=40u8 {
            if !candidates.contains(&v) {
                candidates.push(v);
            }
        }

        for version_num in candidates {
            let dimension = 17 + 4 * version_num as usize;
            for br in &br_candidates {
                let transform = match Self::build_transform(
                    top_left,
                    top_right,
                    bottom_left,
                    br,
                    dimension,
                ) {
                    Some(t) => t,
                    None => continue,
                };
                let transform = Self::refine_transform_with_alignment(
                    binary,
                    &transform,
                    version_num,
                    dimension,
                    module_size,
                    top_left,
                    top_right,
                    bottom_left,
                )
                .unwrap_or(transform);

                let qr_matrix =
                    Self::extract_qr_region_gray_with_transform(gray, width, height, &transform, dimension);
                if let Some(qr) = Self::decode_from_matrix(&qr_matrix, version_num) {
                    return Some(qr);
                }

                let inverted = Self::invert_matrix(&qr_matrix);
                if let Some(qr) = Self::decode_from_matrix(&inverted, version_num) {
                    return Some(qr);
                }

                let qr_matrix = Self::extract_qr_region_with_transform(binary, &transform, dimension);
                if let Some(qr) = Self::decode_from_matrix(&qr_matrix, version_num) {
                    return Some(qr);
                }
            }
        }

        None
    }

    fn calculate_bottom_right(
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
    ) -> Option<Point> {
        // In a perfect QR code, bottom_right = top_right + bottom_left - top_left
        let x = top_right.x + bottom_left.x - top_left.x;
        let y = top_right.y + bottom_left.y - top_left.y;
        Some(Point::new(x, y))
    }

    fn estimate_dimension(
        top_left: &Point,
        top_right: &Point,
        _bottom_right: &Point,
        module_size: f32,
    ) -> Option<usize> {
        // Calculate width in modules
        let width_pixels = top_left.distance(top_right);
        let width_modules = (width_pixels / module_size).round() as i32;

        // QR dimension = width + 7 (for the finder patterns at each end)
        // Actually, width should already include the full QR code
        // For version 1: 21 modules, version 2: 25, etc.
        // dimension = 17 + 4 * version

        // Infer version from measured width
        // version = (dimension - 17) / 4
        // dimension should be approximately width_modules + 7 (finder pattern width)
        let dimension = width_modules + 7;

        #[cfg(debug_assertions)]
        eprintln!(
            "    DECODE: width_pixels={:.1}, width_modules={}, raw_dimension={}",
            width_pixels, width_modules, dimension
        );

        // Minimum valid dimension is 21 (version 1)
        if dimension < 21 {
            #[cfg(debug_assertions)]
            eprintln!("    DECODE: dimension {} < 21, FAIL", dimension);
            return None;
        }

        // Round to nearest valid dimension (must be 21, 25, 29, ... 177)
        let raw_version = ((dimension - 17) as f32 / 4.0).round() as i32;
        let version = raw_version as u8;

        #[cfg(debug_assertions)]
        eprintln!(
            "    DECODE: raw_version={}, final_version={}",
            raw_version, version
        );

        if (1..=40).contains(&version) {
            let final_dim = 17 + 4 * version as usize;
            #[cfg(debug_assertions)]
            eprintln!("    DECODE: final_dimension={}", final_dim);
            Some(final_dim)
        } else {
            #[cfg(debug_assertions)]
            eprintln!("    DECODE: version {} out of range, FAIL", version);
            None
        }
    }

    fn extract_qr_region(
        matrix: &BitMatrix,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        bottom_right: &Point,
        dimension: usize,
    ) -> Option<BitMatrix> {
        let transform = Self::build_transform(
            top_left,
            top_right,
            bottom_left,
            bottom_right,
            dimension,
        )?;
        Some(Self::extract_qr_region_with_transform(
            matrix,
            &transform,
            dimension,
        ))
    }

    fn extract_qr_region_gray(
        gray: &[u8],
        width: usize,
        height: usize,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        bottom_right: &Point,
        dimension: usize,
    ) -> Option<BitMatrix> {
        let transform = Self::build_transform(
            top_left,
            top_right,
            bottom_left,
            bottom_right,
            dimension,
        )?;
        Some(Self::extract_qr_region_gray_with_transform(
            gray,
            width,
            height,
            &transform,
            dimension,
        ))
    }

    fn build_transform(
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
        bottom_right: &Point,
        dimension: usize,
    ) -> Option<PerspectiveTransform> {
        let src = [
            Point::new(3.5, 3.5), // Top-left finder center in module coordinates
            Point::new(dimension as f32 - 3.5, 3.5), // Top-right
            Point::new(3.5, dimension as f32 - 3.5), // Bottom-left
            Point::new(dimension as f32 - 3.5, dimension as f32 - 3.5), // Bottom-right
        ];
        let dst = [*top_left, *top_right, *bottom_left, *bottom_right];
        PerspectiveTransform::from_points(&src, &dst)
    }

    fn extract_qr_region_with_transform(
        matrix: &BitMatrix,
        transform: &PerspectiveTransform,
        dimension: usize,
    ) -> BitMatrix {
        let mut result = BitMatrix::new(dimension, dimension);

        for y in 0..dimension {
            for x in 0..dimension {
                let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
                let img_point = transform.transform(&module_center);

                let img_x = img_point.x.round() as isize;
                let img_y = img_point.y.round() as isize;

                let mut black = 0;
                let mut total = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let sx = img_x + dx;
                        let sy = img_y + dy;
                        if sx >= 0
                            && sy >= 0
                            && (sx as usize) < matrix.width()
                            && (sy as usize) < matrix.height()
                        {
                            total += 1;
                            if matrix.get(sx as usize, sy as usize) {
                                black += 1;
                            }
                        }
                    }
                }
                if total > 0 {
                    result.set(x, y, black * 2 >= total);
                }
            }
        }

        result
    }

    fn extract_qr_region_gray_with_transform(
        gray: &[u8],
        width: usize,
        height: usize,
        transform: &PerspectiveTransform,
        dimension: usize,
    ) -> BitMatrix {
        let mut samples: Vec<u8> = Vec::with_capacity(dimension * dimension);
        for y in 0..dimension {
            for x in 0..dimension {
                let module_center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
                let img_point = transform.transform(&module_center);
                let img_x = img_point.x.round() as isize;
                let img_y = img_point.y.round() as isize;

                let mut sum = 0u32;
                let mut count = 0u32;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let sx = img_x + dx;
                        let sy = img_y + dy;
                        if sx >= 0 && sy >= 0 && (sx as usize) < width && (sy as usize) < height
                        {
                            let idx = sy as usize * width + sx as usize;
                            sum += gray[idx] as u32;
                            count += 1;
                        }
                    }
                }
                let avg = if count > 0 { (sum / count) as u8 } else { 255u8 };
                samples.push(avg);
            }
        }

        let mut sorted = samples.clone();
        sorted.sort_unstable();
        let threshold = sorted[sorted.len() / 2];

        let mut result = BitMatrix::new(dimension, dimension);
        for y in 0..dimension {
            for x in 0..dimension {
                let idx = y * dimension + x;
                result.set(x, y, samples[idx] < threshold);
            }
        }

        result
    }

    fn refine_transform_with_alignment(
        binary: &BitMatrix,
        transform: &PerspectiveTransform,
        version_num: u8,
        dimension: usize,
        module_size: f32,
        top_left: &Point,
        top_right: &Point,
        bottom_left: &Point,
    ) -> Option<PerspectiveTransform> {
        if version_num < 2 || module_size < 1.0 {
            return None;
        }

        let centers = Self::alignment_centers(version_num, dimension);
        let (ax, ay) = centers.iter().max_by_key(|(x, y)| x + y)?;
        let align_src = Point::new(*ax as f32 + 0.5, *ay as f32 + 0.5);
        let predicted = transform.transform(&align_src);
        let found = Self::find_alignment_center(binary, predicted, module_size)?;

        let src = [
            Point::new(3.5, 3.5),
            Point::new(dimension as f32 - 3.5, 3.5),
            Point::new(3.5, dimension as f32 - 3.5),
            align_src,
        ];
        let dst = [*top_left, *top_right, *bottom_left, found];
        PerspectiveTransform::from_points(&src, &dst)
    }

    fn alignment_centers(version: u8, dimension: usize) -> Vec<(usize, usize)> {
        let positions = alignment_pattern_positions(version);
        if positions.is_empty() {
            return Vec::new();
        }

        let mut centers = Vec::new();
        for &cx in &positions {
            for &cy in &positions {
                let in_tl = cx <= 8 && cy <= 8;
                let in_tr = cx >= dimension - 9 && cy <= 8;
                let in_bl = cx <= 8 && cy >= dimension - 9;
                if in_tl || in_tr || in_bl {
                    continue;
                }
                centers.push((cx, cy));
            }
        }
        centers
    }

    fn find_alignment_center(
        binary: &BitMatrix,
        predicted: Point,
        module_size: f32,
    ) -> Option<Point> {
        if !predicted.x.is_finite() || !predicted.y.is_finite() {
            return None;
        }

        let radius = (module_size * 4.0).max(4.0);
        let min_x = (predicted.x - radius).floor().max(0.0) as isize;
        let max_x = (predicted.x + radius)
            .ceil()
            .min((binary.width().saturating_sub(1)) as f32) as isize;
        let min_y = (predicted.y - radius).floor().max(0.0) as isize;
        let max_y = (predicted.y + radius)
            .ceil()
            .min((binary.height().saturating_sub(1)) as f32) as isize;

        let mut best: Option<(Point, usize)> = None;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let center = Point::new(x as f32, y as f32);
                let mismatch = match Self::alignment_pattern_mismatch(binary, &center, module_size) {
                    Some(v) => v,
                    None => continue,
                };
                match best {
                    Some((_, best_mismatch)) if mismatch >= best_mismatch => {}
                    _ => best = Some((center, mismatch)),
                }
            }
        }

        match best {
            Some((center, mismatch)) if mismatch <= 8 => Some(center),
            _ => None,
        }
    }

    fn alignment_pattern_mismatch(
        binary: &BitMatrix,
        center: &Point,
        module_size: f32,
    ) -> Option<usize> {
        let mut mismatches = 0usize;
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let expected_black =
                    dx.abs() == 2 || dy.abs() == 2 || (dx == 0 && dy == 0);
                let sx = center.x + dx as f32 * module_size;
                let sy = center.y + dy as f32 * module_size;
                let ix = sx.round() as isize;
                let iy = sy.round() as isize;
                if ix < 0
                    || iy < 0
                    || (ix as usize) >= binary.width()
                    || (iy as usize) >= binary.height()
                {
                    return None;
                }
                let actual = binary.get(ix as usize, iy as usize);
                if actual != expected_black {
                    mismatches += 1;
                }
            }
        }

        Some(mismatches)
    }

    pub(crate) fn decode_from_matrix(qr_matrix: &BitMatrix, version_num: u8) -> Option<QRCode> {
        let mut best: Option<(QRCode, i32)> = None;

        for oriented in Self::orientations(qr_matrix) {
            let dimension = oriented.width();

            // Build candidate format/mask combos.
            let mut candidates = Vec::new();
            if let Some(info) = FormatInfo::extract(&oriented) {
                candidates.push(info);
            }

            let all_ec = [ECLevel::L, ECLevel::M, ECLevel::Q, ECLevel::H];
            for &ec in &all_ec {
                for mask in 0..8u8 {
                    if let Some(mask_pattern) = crate::models::MaskPattern::from_bits(mask) {
                        let info = FormatInfo {
                            ec_level: ec,
                            mask_pattern,
                        };
                        if !candidates
                            .iter()
                            .any(|c| c.ec_level == ec && c.mask_pattern == mask_pattern)
                        {
                            candidates.push(info);
                        }
                    }
                }
            }

            let traversal_opts = [
                (true, false),
                (true, true),
                (false, false),
                (false, true),
            ];

            for format_info in candidates {
                let func = FunctionMask::new(version_num);
                let mut unmasked = oriented.clone();
                unmask(&mut unmasked, &format_info.mask_pattern, &func);

                for (start_upward, swap_columns) in traversal_opts {
                    let bits = BitstreamExtractor::extract_with_options(
                        &unmasked,
                        dimension,
                        &func,
                        start_upward,
                        swap_columns,
                    );
                    let mut bitstreams = vec![bits.clone()];
                    let mut reversed = bits;
                    reversed.reverse();
                    bitstreams.push(reversed);

                    for stream in bitstreams {
                        for codewords in [
                            Self::bits_to_codewords(&stream),
                            Self::bits_to_codewords_lsb(&stream),
                        ] {
                        let data_codewords = match Self::deinterleave_and_correct(
                            &codewords,
                            version_num,
                            format_info.ec_level,
                        ) {
                            Some(v) => v,
                            None => continue,
                        };

                        if let Some((data, content)) =
                            Self::decode_payload(&data_codewords, version_num)
                        {
                            if data.is_empty() {
                                continue;
                            }
                            let version = if dimension >= 45 {
                                VersionInfo::extract(&oriented)
                                    .map(Version::Model2)
                                    .unwrap_or(Version::Model2(version_num))
                            } else {
                                Version::Model2(version_num)
                            };

                            let score = Self::score_content(&content).max(0);
                            let qr = QRCode::new(
                                data,
                                content,
                                version,
                                format_info.ec_level,
                                format_info.mask_pattern,
                            );
                        match &best {
                            Some((_, best_score)) if *best_score >= score => {}
                            _ => best = Some((qr, score)),
                        }
                        }
                    }
                }
            }
        }
        }

        best.map(|(qr, _)| qr)
    }

    fn score_content(content: &str) -> i32 {
        if content.is_empty() {
            return -10_000;
        }
        if content.chars().all(|c| c.is_ascii_digit()) {
            return 1000 + content.len() as i32 * 10;
        }

        let mut digits = 0;
        let mut alnum = 0;
        let mut printable = 0;
        let mut total = 0;
        let mut non_ascii = 0;
        for ch in content.chars() {
            total += 1;
            if ch.is_ascii_digit() {
                digits += 1;
            }
            if ch.is_ascii_alphanumeric() {
                alnum += 1;
            }
            if ch.is_ascii_graphic() || ch == ' ' {
                printable += 1;
            }
            if !ch.is_ascii() {
                non_ascii += 1;
            }
        }
        let non_print = total - printable;
        let mut score = digits * 5 + alnum * 2 + printable - non_print * 5 - non_ascii * 10;
        if digits * 2 >= total {
            score += 50;
        }
        score as i32
    }

    fn orientations(matrix: &BitMatrix) -> Vec<BitMatrix> {
        let mut out = Vec::new();
        let r0 = matrix.clone();
        let r90 = Self::rotate90(&r0);
        let r180 = Self::rotate180(&r0);
        let r270 = Self::rotate270(&r0);
        let fh = Self::flip_horizontal(&r0);
        let fv = Self::flip_vertical(&r0);
        let fhr90 = Self::rotate90(&fh);
        let fvr90 = Self::rotate90(&fv);

        out.push(r0);
        out.push(r90);
        out.push(r180);
        out.push(r270);
        out.push(fh);
        out.push(fv);
        out.push(fhr90);
        out.push(fvr90);

        out
    }

    fn rotate90(matrix: &BitMatrix) -> BitMatrix {
        let n = matrix.width();
        let mut out = BitMatrix::new(n, n);
        for y in 0..n {
            for x in 0..n {
                out.set(n - 1 - y, x, matrix.get(x, y));
            }
        }
        out
    }

    fn rotate180(matrix: &BitMatrix) -> BitMatrix {
        let n = matrix.width();
        let mut out = BitMatrix::new(n, n);
        for y in 0..n {
            for x in 0..n {
                out.set(n - 1 - x, n - 1 - y, matrix.get(x, y));
            }
        }
        out
    }

    fn rotate270(matrix: &BitMatrix) -> BitMatrix {
        let n = matrix.width();
        let mut out = BitMatrix::new(n, n);
        for y in 0..n {
            for x in 0..n {
                out.set(y, n - 1 - x, matrix.get(x, y));
            }
        }
        out
    }

    fn flip_horizontal(matrix: &BitMatrix) -> BitMatrix {
        let n = matrix.width();
        let mut out = BitMatrix::new(n, n);
        for y in 0..n {
            for x in 0..n {
                out.set(n - 1 - x, y, matrix.get(x, y));
            }
        }
        out
    }

    fn flip_vertical(matrix: &BitMatrix) -> BitMatrix {
        let n = matrix.width();
        let mut out = BitMatrix::new(n, n);
        for y in 0..n {
            for x in 0..n {
                out.set(x, n - 1 - y, matrix.get(x, y));
            }
        }
        out
    }

    fn invert_matrix(matrix: &BitMatrix) -> BitMatrix {
        let mut out = BitMatrix::new(matrix.width(), matrix.height());
        for y in 0..matrix.height() {
            for x in 0..matrix.width() {
                out.set(x, y, !matrix.get(x, y));
            }
        }
        out
    }

    fn bits_to_codewords(bits: &[bool]) -> Vec<u8> {
        let mut codewords = Vec::with_capacity(bits.len() / 8);
        let mut idx = 0;
        while idx + 8 <= bits.len() {
            let mut byte = 0u8;
            for _ in 0..8 {
                byte = (byte << 1) | (bits[idx] as u8);
                idx += 1;
            }
            codewords.push(byte);
        }
        codewords
    }

    fn bits_to_codewords_lsb(bits: &[bool]) -> Vec<u8> {
        let mut codewords = Vec::with_capacity(bits.len() / 8);
        let mut idx = 0;
        while idx + 8 <= bits.len() {
            let mut byte = 0u8;
            for bit in 0..8 {
                if bits[idx] {
                    byte |= 1 << bit;
                }
                idx += 1;
            }
            codewords.push(byte);
        }
        codewords
    }

    fn deinterleave_and_correct(
        codewords: &[u8],
        version: u8,
        ec_level: ECLevel,
    ) -> Option<Vec<u8>> {
        let info = ec_block_info(version, ec_level)?;
        let total = codewords.len();
        let ecc_total = info.num_blocks * info.ecc_per_block;
        if total < ecc_total {
            return None;
        }
        let data_total = total - ecc_total;
        if data_total == 0 {
            return None;
        }

        let num_long_blocks = data_total % info.num_blocks;
        let num_short_blocks = info.num_blocks - num_long_blocks;
        let short_len = data_total / info.num_blocks;
        let long_len = short_len + 1;

        let mut blocks: Vec<Vec<u8>> = (0..info.num_blocks)
            .map(|_| Vec::with_capacity(long_len + info.ecc_per_block))
            .collect();

        let mut idx = 0;
        for i in 0..long_len {
            for b in 0..info.num_blocks {
                let block_len = if b < num_short_blocks {
                    short_len
                } else {
                    long_len
                };
                if i < block_len {
                    if idx >= total {
                        return None;
                    }
                    blocks[b].push(codewords[idx]);
                    idx += 1;
                }
            }
        }

        for _ in 0..info.ecc_per_block {
            for b in 0..info.num_blocks {
                if idx >= total {
                    return None;
                }
                blocks[b].push(codewords[idx]);
                idx += 1;
            }
        }

        let rs = ReedSolomonDecoder::new(info.ecc_per_block);
        let mut data_out = Vec::with_capacity(data_total);
        for (b, block) in blocks.iter_mut().enumerate() {
            if rs.decode(block).is_err() {
                return None;
            }
            let data_len = if b < num_short_blocks {
                short_len
            } else {
                long_len
            };
            data_out.extend_from_slice(&block[..data_len]);
        }

        Some(data_out)
    }

    fn decode_payload(data_codewords: &[u8], version: u8) -> Option<(Vec<u8>, String)> {
        let mut bits = Vec::with_capacity(data_codewords.len() * 8);
        for &byte in data_codewords {
            for i in (0..8).rev() {
                bits.push(((byte >> i) & 1) != 0);
            }
        }

        Self::decode_payload_from_bits(&bits, version)
    }

    fn decode_payload_from_bits(bits: &[bool], version: u8) -> Option<(Vec<u8>, String)> {
        let mut reader = BitReader::new(bits);
        let mut data = Vec::new();
        let mut content = String::new();

        loop {
            if reader.remaining() < 4 {
                break;
            }
            let mode = reader.read_bits(4)? as u8;
            if mode == 0 {
                break;
            }

            match mode {
                1 => {
                    let count_bits = char_count_bits(mode, version);
                    let count = reader.read_bits(count_bits)? as usize;
                    let start = reader.index();
                    let (decoded, used) = NumericDecoder::decode(&bits[start..], count)?;
                    reader.advance(used);
                    data.extend_from_slice(decoded.as_bytes());
                    content.push_str(&decoded);
                }
                2 => {
                    let count_bits = char_count_bits(mode, version);
                    let count = reader.read_bits(count_bits)? as usize;
                    let start = reader.index();
                    let (decoded, used) = AlphanumericDecoder::decode(&bits[start..], count)?;
                    reader.advance(used);
                    data.extend_from_slice(decoded.as_bytes());
                    content.push_str(&decoded);
                }
                4 => {
                    let count_bits = char_count_bits(mode, version);
                    let count = reader.read_bits(count_bits)? as usize;
                    let mut bytes = Vec::with_capacity(count);
                    for _ in 0..count {
                        let byte = reader.read_bits(8)? as u8;
                        bytes.push(byte);
                    }
                    data.extend_from_slice(&bytes);
                    content.push_str(&String::from_utf8_lossy(&bytes));
                }
                7 => {
                    // ECI: parse and ignore for now (assume UTF-8)
                    let mut eci = reader.read_bits(8)? as u32;
                    if (eci & 0x80) != 0 {
                        eci = ((eci & 0x7F) << 8) | reader.read_bits(8)?;
                        if (eci & 0x4000) != 0 {
                            eci = ((eci & 0x3FFF) << 8) | reader.read_bits(8)?;
                        }
                    }
                    let _ = eci;
                }
                _ => return None,
            }
        }

        Some((data, content))
    }
}

struct BitReader<'a> {
    bits: &'a [bool],
    idx: usize,
}

impl<'a> BitReader<'a> {
    fn new(bits: &'a [bool]) -> Self {
        Self { bits, idx: 0 }
    }

    fn remaining(&self) -> usize {
        self.bits.len().saturating_sub(self.idx)
    }

    fn index(&self) -> usize {
        self.idx
    }

    fn advance(&mut self, n: usize) {
        self.idx = (self.idx + n).min(self.bits.len());
    }

    fn read_bits(&mut self, n: usize) -> Option<u32> {
        if self.idx + n > self.bits.len() {
            return None;
        }
        let mut val = 0u32;
        for _ in 0..n {
            val = (val << 1) | (self.bits[self.idx] as u32);
            self.idx += 1;
        }
        Some(val)
    }
}

fn char_count_bits(mode: u8, version: u8) -> usize {
    let ver = version as usize;
    match mode {
        1 => {
            if ver <= 9 {
                10
            } else if ver <= 26 {
                12
            } else {
                14
            }
        }
        2 => {
            if ver <= 9 {
                9
            } else if ver <= 26 {
                11
            } else {
                13
            }
        }
        4 => {
            if ver <= 9 {
                8
            } else {
                16
            }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_basic() {
        // Create a simple test case with 3 finder patterns
        let matrix = BitMatrix::new(100, 100);
        let tl = Point::new(20.0, 20.0);
        let tr = Point::new(80.0, 20.0);
        let bl = Point::new(20.0, 80.0);

        // This will fail because there's no actual QR code in the matrix
        // but it exercises the decode pipeline (smoke test)
        let _ = QrDecoder::decode(&matrix, &tl, &tr, &bl, 1.0);
    }

    #[test]
    fn test_decode_payload_byte_mode() {
        // Byte mode, version 1: "HI"
        let mut bits = Vec::new();
        push_bits(&mut bits, 0b0100, 4); // mode
        push_bits(&mut bits, 2, 8); // count
        push_bits(&mut bits, b'H' as u32, 8);
        push_bits(&mut bits, b'I' as u32, 8);
        push_bits(&mut bits, 0, 4); // terminator

        let codewords = QrDecoder::bits_to_codewords(&bits);
        let (data, content) = QrDecoder::decode_payload(&codewords, 1).unwrap();
        assert_eq!(content, "HI");
        assert_eq!(data, b"HI");
    }

    fn push_bits(bits: &mut Vec<bool>, value: u32, count: usize) {
        for i in (0..count).rev() {
            bits.push(((value >> i) & 1) != 0);
        }
    }

    #[test]
    fn test_golden_matrix_decode() {
        // Known-good 21x21 QR matrix for "4376471154038" (Version 1-M)
        // Generated with Python qrcode library
        let grid: [[bool; 21]; 21] = [
            [true, true, true, true, true, true, true, false, false, false, false, false, true, false, true, true, true, true, true, true, true],
            [true, false, false, false, false, false, true, false, false, true, false, false, false, false, true, false, false, false, false, false, true],
            [true, false, true, true, true, false, true, false, false, false, true, true, false, false, true, false, true, true, true, false, true],
            [true, false, true, true, true, false, true, false, false, false, true, false, false, false, true, false, true, true, true, false, true],
            [true, false, true, true, true, false, true, false, false, true, true, true, true, false, true, false, true, true, true, false, true],
            [true, false, false, false, false, false, true, false, true, false, true, false, false, false, true, false, false, false, false, false, true],
            [true, true, true, true, true, true, true, false, true, false, true, false, true, false, true, true, true, true, true, true, true],
            [false, false, false, false, false, false, false, false, false, true, false, false, false, false, false, false, false, false, false, false, false],
            [true, false, false, true, false, true, true, false, true, true, true, true, true, true, false, true, false, false, false, false, false],
            [true, true, true, false, true, false, false, true, true, false, false, true, false, true, false, true, false, true, true, false, false],
            [true, false, false, true, false, true, true, true, true, false, true, true, false, false, true, true, true, false, false, false, true],
            [false, false, true, false, true, false, false, true, false, false, false, false, true, true, true, true, true, false, false, false, false],
            [false, false, true, false, false, false, true, true, false, true, false, true, false, true, true, true, false, true, true, false, false],
            [false, false, false, false, false, false, false, false, true, false, true, false, false, true, true, true, true, false, true, true, false],
            [true, true, true, true, true, true, true, false, false, false, true, true, true, false, true, false, true, true, true, true, false],
            [true, false, false, false, false, false, true, false, true, false, false, false, false, false, true, true, false, false, false, false, true],
            [true, false, true, true, true, false, true, false, false, true, true, false, true, true, true, false, false, true, false, true, true],
            [true, false, true, true, true, false, true, false, true, false, true, false, false, true, true, true, true, false, false, true, true],
            [true, false, true, true, true, false, true, false, false, true, true, true, false, true, true, true, false, true, false, false, true],
            [true, false, false, false, false, false, true, false, false, true, true, true, true, false, false, true, true, false, false, true, false],
            [true, true, true, true, true, true, true, false, true, true, true, false, false, true, false, true, true, true, false, false, false],
        ];

        let mut matrix = BitMatrix::new(21, 21);
        for y in 0..21 {
            for x in 0..21 {
                matrix.set(x, y, grid[y][x]);
            }
        }

        let result = QrDecoder::decode_from_matrix(&matrix, 1);
        assert!(result.is_some(), "Failed to decode golden QR matrix");
        let qr = result.unwrap();
        assert_eq!(qr.content, "4376471154038");
    }
}
