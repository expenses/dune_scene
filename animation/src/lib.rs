use gltf::animation::Interpolation;
use ultraviolet::{Rotor3, Similarity3, Vec3};

pub fn read_animations(
    animations: gltf::iter::Animations,
    gltf_binary_buffer_blob: &[u8],
    model_name: &str,
) -> Vec<Animation> {
    animations
        .map(|animation| {
            let mut translation_channels = Vec::new();
            let mut rotation_channels = Vec::new();
            let mut scale_channels = Vec::new();

            for (channel_index, channel) in animation.channels().enumerate() {
                let reader = channel.reader(|buffer| {
                    assert_eq!(buffer.index(), 0);
                    Some(gltf_binary_buffer_blob)
                });

                let inputs = reader.read_inputs().unwrap().collect();

                log::trace!(
                    "[{}] animation {:?}, channel {} ({:?}) uses {:?} interpolation.",
                    model_name,
                    animation.name(),
                    channel_index,
                    channel.target().property(),
                    channel.sampler().interpolation()
                );

                match channel.target().property() {
                    gltf::animation::Property::Translation => {
                        let outputs = match reader.read_outputs().unwrap() {
                            gltf::animation::util::ReadOutputs::Translations(translations) => {
                                translations.map(|translation| translation.into()).collect()
                            }
                            _ => unreachable!(),
                        };

                        translation_channels.push(Channel {
                            interpolation: channel.sampler().interpolation(),
                            inputs,
                            outputs,
                            node_index: channel.target().node().index(),
                        });
                    }
                    gltf::animation::Property::Rotation => {
                        let outputs = match reader.read_outputs().unwrap() {
                            gltf::animation::util::ReadOutputs::Rotations(rotations) => rotations
                                .into_f32()
                                .map(Rotor3::from_quaternion_array)
                                .collect(),
                            _ => unreachable!(),
                        };

                        rotation_channels.push(Channel {
                            interpolation: channel.sampler().interpolation(),
                            inputs,
                            outputs,
                            node_index: channel.target().node().index(),
                        });
                    }
                    gltf::animation::Property::Scale => {
                        let outputs = match reader.read_outputs().unwrap() {
                            gltf::animation::util::ReadOutputs::Scales(scales) => scales
                                .map(|scales| (scales[0] + scales[1] + scales[2]) / 3.0)
                                .collect(),
                            _ => unreachable!(),
                        };

                        scale_channels.push(Channel {
                            interpolation: channel.sampler().interpolation(),
                            inputs,
                            outputs,
                            node_index: channel.target().node().index(),
                        });
                    }
                    property => {
                        log::warn!(
                            "[{}] Animation type {:?} is not supported, ignoring.",
                            model_name,
                            property
                        );
                    }
                }
            }

            let total_time = translation_channels
                .iter()
                .map(|channel| channel.inputs[channel.inputs.len() - 1])
                .chain(
                    rotation_channels
                        .iter()
                        .map(|channel| channel.inputs[channel.inputs.len() - 1]),
                )
                .chain(
                    scale_channels
                        .iter()
                        .map(|channel| channel.inputs[channel.inputs.len() - 1]),
                )
                .max_by_key(|&time| ordered_float::OrderedFloat(time))
                .unwrap();

            Animation {
                total_time,
                translation_channels,
                rotation_channels,
                scale_channels,
            }
        })
        .collect()
}

pub fn initial_local_transforms_from_nodes(nodes: gltf::iter::Nodes) -> Vec<Similarity3> {
    nodes
        .map(|node| {
            let (translation, rotation, scale) = node.transform().decomposed();
            let translation = Vec3::from(translation);
            let rotation = Rotor3::from_quaternion_array(rotation);
            let scale = (scale[0] + scale[1] + scale[2]) / 3.0;

            Similarity3::new(translation, rotation, scale)
        })
        .collect()
}

#[derive(Debug)]
pub struct Channel<T> {
    pub interpolation: Interpolation,
    pub inputs: Vec<f32>,
    pub outputs: Vec<T>,
    pub node_index: usize,
}

#[derive(Debug)]
pub struct Animation {
    pub total_time: f32,
    pub translation_channels: Vec<Channel<Vec3>>,
    pub rotation_channels: Vec<Channel<Rotor3>>,
    pub scale_channels: Vec<Channel<f32>>,
}
