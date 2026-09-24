//! Gaussian splat stimulus <-> proto conversions.

use std::path::Path;

use crate::ipc::response::err;
use crate::proto;
use crate::scene::stimulus::GaussianSplat;
use crate::splat::SplatFileInfo;

use super::mesh3d::Refusal;

/// Validate a create request's params: the file must exist and be a splat file
/// this server reads. Reads only the header, so it is cheap on the ZMQ thread.
pub(crate) fn gaussian_splat3d_from_proto(
    p: proto::GaussianSplat3DParams,
) -> Result<(String, SplatFileInfo), Refusal> {
    if p.path.is_empty() {
        return Err(Box::new(err(
            proto::ErrorCode::InvalidArgument,
            "GaussianSplat3D needs a path",
        )));
    }
    match crate::splat::probe(Path::new(&p.path)) {
        Ok(info) => Ok((p.path, info)),
        // Every failure is the request's path being wrong. FILE_NOT_FOUND and
        // FILE_IO mean a scene-config to clients, which map them to its errors.
        Err(e) => Err(Box::new(err(proto::ErrorCode::InvalidArgument, e.to_string()))),
    }
}

pub(crate) fn gaussian_splat3d_params_to_proto(g: &GaussianSplat) -> proto::stimulus_params::Shape {
    proto::stimulus_params::Shape::GaussianSplat3d(proto::GaussianSplat3DParams {
        path: g.path.clone(),
    })
}
