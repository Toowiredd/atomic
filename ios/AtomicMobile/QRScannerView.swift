import SwiftUI
import UIKit
import AVFoundation

struct QRPayload {
    let url: String
    let token: String
}

struct QRScannerView: UIViewControllerRepresentable {
    @Binding var scannedPayload: QRPayload?
    @Environment(\.dismiss) private var dismiss

    func makeUIViewController(context: Context) -> QRScannerViewController {
        let controller = QRScannerViewController()
        controller.delegate = context.coordinator
        return controller
    }

    func updateUIViewController(_ uiViewController: QRScannerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    final class Coordinator: NSObject, QRScannerDelegate, @unchecked Sendable {
        let parent: QRScannerView

        init(_ parent: QRScannerView) {
            self.parent = parent
        }

        func didScanQRCode(_ payload: QRPayload) {
            Task { @MainActor in
                parent.scannedPayload = payload
                parent.dismiss()
            }
        }

        func didFailWithError(_ error: String) {
            Task { @MainActor in
                parent.dismiss()
            }
        }
    }
}

protocol QRScannerDelegate: AnyObject {
    func didScanQRCode(_ payload: QRPayload)
    func didFailWithError(_ error: String)
}

final class QRScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    weak var delegate: QRScannerDelegate?

    private let captureSession = AVCaptureSession()
    private var previewLayer: AVCaptureVideoPreviewLayer?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        setupCamera()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.bounds
    }

    private func setupCamera() {
        guard let videoCaptureDevice = AVCaptureDevice.default(for: .video) else {
            delegate?.didFailWithError("No camera available")
            return
        }

        guard let videoInput = try? AVCaptureDeviceInput(device: videoCaptureDevice) else {
            delegate?.didFailWithError("Could not create video input")
            return
        }

        if captureSession.canAddInput(videoInput) {
            captureSession.addInput(videoInput)
        } else {
            delegate?.didFailWithError("Could not add video input")
            return
        }

        let metadataOutput = AVCaptureMetadataOutput()
        if captureSession.canAddOutput(metadataOutput) {
            captureSession.addOutput(metadataOutput)
            metadataOutput.setMetadataObjectsDelegate(self, queue: DispatchQueue.main)
            metadataOutput.metadataObjectTypes = [.qr]
        } else {
            delegate?.didFailWithError("Could not add metadata output")
            return
        }

        let preview = AVCaptureVideoPreviewLayer(session: captureSession)
        preview.frame = view.bounds
        preview.videoGravity = .resizeAspectFill
        view.layer.addSublayer(preview)
        previewLayer = preview

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            self?.captureSession.startRunning()
        }
    }

    nonisolated func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard let metadataObject = metadataObjects.first,
              let readableObject = metadataObject as? AVMetadataMachineReadableCodeObject,
              let stringValue = readableObject.stringValue else {
            return
        }

        // Parse JSON payload: {"url":"...","token":"..."}
        guard let data = stringValue.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: String],
              let url = json["url"],
              let token = json["token"] else {
            return
        }

        let payload = QRPayload(url: url, token: token)

        Task { @MainActor [weak self] in
            self?.captureSession.stopRunning()
            self?.delegate?.didScanQRCode(payload)
        }
    }
}
