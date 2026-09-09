use crate::vec3::{v3, Vec3};

#[derive(Clone)]
pub struct Material {
    pub name: &'static str,
    pub albedo: Vec3,
    pub texture: Option<usize>,
    pub normal_map: Option<usize>,
    pub kd: f32,
    pub ks: f32,
    pub shininess: f32,
    pub reflectivity: f32,
    pub transparency: f32,
    pub ior: f32,
    pub emission: Vec3,
}

impl Default for Material {
    fn default() -> Material {
        Material {
            name: "material",
            albedo: Vec3::ONE,
            texture: None,
            normal_map: None,
            kd: 0.9,
            ks: 0.1,
            shininess: 16.0,
            reflectivity: 0.0,
            transparency: 0.0,
            ior: 1.0,
            emission: Vec3::ZERO,
        }
    }
}

impl Material {
    pub fn opaque(name: &'static str, texture: usize) -> Material {
        Material {
            name,
            texture: Some(texture),
            ..Material::default()
        }
    }

    pub fn with_normal_map(mut self, nm: usize) -> Material {
        self.normal_map = Some(nm);
        self
    }

    pub fn with_phong(mut self, kd: f32, ks: f32, shininess: f32) -> Material {
        self.kd = kd;
        self.ks = ks;
        self.shininess = shininess;
        self
    }

    pub fn with_reflectivity(mut self, r: f32) -> Material {
        self.reflectivity = r;
        self
    }

    pub fn with_refraction(mut self, transparency: f32, ior: f32) -> Material {
        self.transparency = transparency;
        self.ior = ior;
        self
    }

    pub fn with_emission(mut self, color: Vec3, strength: f32) -> Material {
        self.emission = color * strength;
        self
    }

    pub fn with_albedo(mut self, a: Vec3) -> Material {
        self.albedo = a;
        self
    }

    pub fn is_emissive(&self) -> bool {
        self.emission.max_comp() > 0.0
    }
}

pub fn fresnel_schlick(cos_theta: f32, ior: f32) -> f32 {
    let r0 = ((1.0 - ior) / (1.0 + ior)).powi(2);
    r0 + (1.0 - r0) * (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5)
}

pub fn tangent_frame(n: Vec3) -> (Vec3, Vec3) {
    if n.x.abs() > 0.5 {
        (v3(0.0, 0.0, -n.x.signum()), v3(0.0, -1.0, 0.0))
    } else if n.y.abs() > 0.5 {
        (v3(1.0, 0.0, 0.0), v3(0.0, 0.0, n.y.signum()))
    } else {
        (v3(n.z.signum(), 0.0, 0.0), v3(0.0, -1.0, 0.0))
    }
}