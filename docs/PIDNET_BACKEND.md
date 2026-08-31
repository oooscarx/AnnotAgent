# PIDNet Backend

PIDNet is represented through the generic `SemanticSegmentation` Capability. A conforming Worker
declares Image input and a typed segmentation Artifact output through its Expert Model Manifest;
Core and Runtime do not switch on the PIDNet brand.

Use **Settings → Vision Workers → Add expert model → PIDNet** to create the setup Draft. AnnotAgent
then requires health, protocol, contracts, immutable model identity, checkpoint/license evidence
and a selected-image conversion before the Worker can be registered.

This repository provides the capability contract and scaffold preset, not a tracked PIDNet
implementation or checkpoint. Real PIDNet inference is `LIVE-CONDITIONAL` and must not be inferred
from the presence of the preset.
