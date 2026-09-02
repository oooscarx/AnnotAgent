# Publishing a Model Bundle

Bundle publishing is a supply-chain task, not a model conversion feature in AnnotAgent.

1. Obtain model files under terms that permit the intended redistribution/use.
2. Record an immutable upstream source revision and hashes; record the exact exporter/version/opset
   and transformation procedure outside the product runtime.
3. Author and test the Plugin model Contract first, then create a matching versioned tensor
   Contract and fixed test vector.
4. Include exact license text and source notice. Do not mark unknown or prohibited redistribution
   as publishable.
5. Pack and verify deterministically; test the package with the target installed Plugin/platform.
6. Create a Catalog entry pinned to package size/digest and audited publisher/license metadata.
7. Sign and host through organizational release controls if required.

AnnotAgent does not run Python, pip, exporters, converters, or arbitrary scripts during pack,
install, inspect, Contract validation, or inference. A raw upstream ONNX pair is not a supported
Bundle until this evidence exists.
