/// Reed-Solomon error correction for QR codes
/// QR codes use RS over GF(256) with primitive polynomial x^8 + x^4 + x^3 + x^2 + 1
/// GF(256) field operations using log/exp tables
pub struct Gf256;

static LOG_TABLE: [u8; 256] = [
    0, 0, 1, 25, 2, 50, 26, 198, 3, 223, 51, 238, 27, 104, 199, 75, 4, 100, 224, 14, 52, 141, 239,
    129, 28, 193, 105, 248, 200, 8, 76, 113, 5, 138, 101, 47, 225, 36, 15, 33, 53, 147, 142, 218,
    240, 18, 130, 69, 29, 181, 194, 125, 106, 39, 249, 185, 201, 154, 9, 120, 77, 228, 114, 166, 6,
    191, 139, 98, 102, 221, 48, 253, 226, 152, 37, 179, 16, 145, 34, 136, 54, 208, 148, 206, 143,
    150, 219, 189, 241, 210, 19, 92, 131, 56, 70, 64, 30, 66, 182, 163, 195, 72, 126, 110, 107, 58,
    40, 84, 250, 133, 186, 61, 202, 94, 155, 159, 10, 21, 121, 43, 78, 212, 229, 172, 115, 243,
    167, 87, 7, 112, 192, 247, 140, 128, 99, 13, 103, 74, 222, 237, 49, 197, 254, 24, 227, 165,
    153, 119, 38, 184, 180, 124, 17, 68, 146, 217, 35, 32, 137, 46, 55, 63, 209, 91, 149, 188, 207,
    205, 144, 135, 151, 178, 220, 252, 190, 97, 242, 86, 211, 171, 20, 42, 93, 158, 132, 60, 57,
    83, 71, 109, 65, 162, 31, 45, 67, 216, 183, 123, 164, 118, 196, 23, 73, 236, 127, 12, 111, 246,
    108, 161, 59, 82, 41, 157, 85, 170, 251, 96, 134, 177, 187, 204, 62, 90, 203, 89, 95, 176, 156,
    169, 160, 81, 11, 245, 22, 235, 122, 117, 44, 215, 79, 174, 213, 233, 230, 231, 173, 232, 116,
    214, 244, 234, 168, 80, 88, 175,
];

static EXP_TABLE: [u8; 256] = [
    1, 2, 4, 8, 16, 32, 64, 128, 29, 58, 116, 232, 205, 135, 19, 38, 76, 152, 45, 90, 180, 117,
    234, 201, 143, 3, 6, 12, 24, 48, 96, 192, 157, 39, 78, 156, 37, 74, 148, 53, 106, 212, 181,
    119, 238, 193, 159, 35, 70, 140, 5, 10, 20, 40, 80, 160, 93, 186, 105, 210, 185, 111, 222, 161,
    95, 190, 97, 194, 153, 47, 94, 188, 101, 202, 137, 15, 30, 60, 120, 240, 253, 231, 211, 187,
    107, 214, 177, 127, 254, 225, 223, 163, 91, 182, 113, 226, 217, 175, 67, 134, 17, 34, 68, 136,
    13, 26, 52, 104, 208, 189, 103, 206, 129, 31, 62, 124, 248, 237, 199, 147, 59, 118, 236, 197,
    151, 51, 102, 204, 133, 23, 46, 92, 184, 109, 218, 169, 71, 142, 1, 2, 4, 8, 16, 32, 64, 128,
    29, 58, 116, 232, 205, 135, 19, 38, 76, 152, 45, 90, 180, 117, 234, 201, 143, 3, 6, 12, 24, 48,
    96, 192, 157, 39, 78, 156, 37, 74, 148, 53, 106, 212, 181, 119, 238, 193, 159, 35, 70, 140, 5,
    10, 20, 40, 80, 160, 93, 186, 105, 210, 185, 111, 222, 161, 95, 190, 97, 194, 153, 47, 94, 188,
    101, 202, 137, 15, 30, 60, 120, 240, 253, 231, 211, 187, 107, 214, 177, 127, 254, 225, 223,
    163, 91, 182, 113, 226, 217, 175, 67, 134, 17, 34, 68, 136, 13, 26, 52, 104, 208, 189, 103,
    206, 129, 31, 62, 124, 248, 237,
];

impl Gf256 {
    pub fn mul(a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        let log_a = LOG_TABLE[a as usize] as usize;
        let log_b = LOG_TABLE[b as usize] as usize;
        EXP_TABLE[(log_a + log_b) % 255]
    }

    pub fn div(a: u8, b: u8) -> u8 {
        if b == 0 {
            panic!("Division by zero");
        }
        if a == 0 {
            return 0;
        }
        let log_a = LOG_TABLE[a as usize] as usize;
        let log_b = LOG_TABLE[b as usize] as usize;
        let diff = if log_a >= log_b {
            log_a - log_b
        } else {
            log_a + 255 - log_b
        };
        EXP_TABLE[diff]
    }

    pub fn pow(a: u8, n: u8) -> u8 {
        if n == 0 {
            return 1;
        }
        if a == 0 {
            return 0;
        }
        let log_a = LOG_TABLE[a as usize] as usize;
        EXP_TABLE[(log_a * n as usize) % 255]
    }
}

/// Reed-Solomon decoder for QR codes
pub struct ReedSolomonDecoder {
    num_ecc_codewords: usize,
}

impl ReedSolomonDecoder {
    pub fn new(num_ecc_codewords: usize) -> Self {
        Self { num_ecc_codewords }
    }

    pub fn decode(&self, received: &mut [u8]) -> Result<(), &'static str> {
        // Calculate syndrome
        let syndrome = self.calculate_syndrome(received);

        // Check if syndrome is zero (no errors)
        let has_errors = syndrome.iter().any(|&s| s != 0);
        if !has_errors {
            return Ok(());
        }

        // Find error locator polynomial using Berlekamp-Massey
        let sigma = self.find_error_locator(&syndrome);

        // Find error positions (Chien search)
        let error_positions = self.find_error_positions(&sigma, received.len())?;

        // Find error values (Forney algorithm)
        let error_values = self.find_error_values(&sigma, &syndrome, &error_positions)?;

        // Correct errors
        for (i, &pos) in error_positions.iter().enumerate() {
            received[pos] ^= error_values[i];
        }

        // Verify syndrome is now zero
        let new_syndrome = self.calculate_syndrome(received);
        if new_syndrome.iter().any(|&s| s != 0) {
            return Err("Uncorrectable error");
        }

        Ok(())
    }

    fn calculate_syndrome(&self, received: &[u8]) -> Vec<u8> {
        let n = received.len();
        let mut syndrome = vec![0u8; self.num_ecc_codewords];

        for (i, syndrome_i) in syndrome.iter_mut().enumerate().take(self.num_ecc_codewords) {
            let mut sum = 0u8;
            for (j, &received_j) in received.iter().enumerate().take(n) {
                let term = Gf256::mul(received_j, Gf256::pow(2, (i * j) as u8));
                sum ^= term;
            }
            *syndrome_i = sum;
        }

        syndrome
    }

    fn find_error_locator(&self, syndrome: &[u8]) -> Vec<u8> {
        // Berlekamp-Massey algorithm
        let n = syndrome.len();
        let mut sigma = vec![1u8];
        let mut b = vec![1u8];
        let mut l = 0;
        let mut m = 1;

        for i in 0..n {
            let mut delta = syndrome[i];
            for j in 1..=l {
                if j < sigma.len() && i >= j {
                    delta ^= Gf256::mul(sigma[j], syndrome[i - j]);
                }
            }

            if delta == 0 {
                m += 1;
            } else if 2 * l <= i {
                let sigma_new = sigma.clone();
                let d = Gf256::div(delta, b[0]);

                // Extend sigma if needed
                while sigma.len() < b.len() + m {
                    sigma.push(0);
                }

                // sigma = sigma - d * x^m * b
                for j in 0..b.len() {
                    let term = Gf256::mul(d, b[j]);
                    if j + m < sigma.len() {
                        sigma[j + m] ^= term;
                    }
                }

                b = sigma_new;
                l = i + 1 - l;
                m = 1;
            } else {
                let d = Gf256::div(delta, b[0]);

                // Extend sigma if needed
                while sigma.len() < b.len() + m {
                    sigma.push(0);
                }

                for j in 0..b.len() {
                    let term = Gf256::mul(d, b[j]);
                    if j + m < sigma.len() {
                        sigma[j + m] ^= term;
                    }
                }

                m += 1;
            }
        }

        sigma
    }

    fn find_error_positions(&self, sigma: &[u8], n: usize) -> Result<Vec<usize>, &'static str> {
        let mut positions = Vec::new();

        // Chien search
        for i in 0..n {
            let x = Gf256::pow(2, (n - 1 - i) as u8);
            let mut sum = 0u8;

            for (j, &coeff) in sigma.iter().enumerate() {
                let term = Gf256::mul(coeff, Gf256::pow(x, j as u8));
                sum ^= term;
            }

            if sum == 0 {
                positions.push(i);
            }
        }

        if positions.len() != sigma.len() - 1 {
            return Err("Wrong number of error positions found");
        }

        Ok(positions)
    }

    fn find_error_values(
        &self,
        sigma: &[u8],
        syndrome: &[u8],
        error_positions: &[usize],
    ) -> Result<Vec<u8>, &'static str> {
        // Forney algorithm
        // omega = syndrome * sigma mod x^(2t)
        let mut omega = vec![0u8; syndrome.len()];
        for i in 0..syndrome.len() {
            for j in 0..=i {
                if j < sigma.len() && i - j < syndrome.len() {
                    omega[i] ^= Gf256::mul(sigma[j], syndrome[i - j]);
                }
            }
        }

        let mut values = Vec::with_capacity(error_positions.len());

        for &pos in error_positions {
            let x = Gf256::pow(2, (pos as u8).wrapping_neg());

            // Evaluate omega at x
            let mut omega_val = 0u8;
            for (i, &coeff) in omega.iter().enumerate() {
                let term = Gf256::mul(coeff, Gf256::pow(x, i as u8));
                omega_val ^= term;
            }

            // Evaluate sigma' at x
            let mut sigma_prime_val = 0u8;
            for (i, &coeff) in sigma.iter().enumerate().skip(1) {
                if i % 2 == 1 {
                    let term = Gf256::mul(coeff, Gf256::pow(x, (i - 1) as u8));
                    sigma_prime_val ^= term;
                }
            }

            let error_value = Gf256::div(omega_val, sigma_prime_val);
            values.push(error_value);
        }

        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gf256_basic() {
        // 0 * anything = 0
        assert_eq!(Gf256::mul(0, 5), 0);
        assert_eq!(Gf256::mul(5, 0), 0);

        // 0 / anything = 0 (except division by 0)
        assert_eq!(Gf256::div(0, 5), 0);

        // x / x = 1 (for x != 0)
        assert_eq!(Gf256::div(7, 7), 1);
        assert_eq!(Gf256::div(123, 123), 1);
    }

    #[test]
    fn test_rs_decode() {
        // Simple test: all zeros should have zero syndrome
        let mut data = vec![0u8; 16];
        let decoder = ReedSolomonDecoder::new(10);

        // All zeros should be valid (no errors to correct)
        assert!(decoder.decode(&mut data).is_ok());
        assert_eq!(data, vec![0u8; 16]);
    }
}
