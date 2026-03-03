import SwiftUI

@main
struct AtomicApp: App {
    @State private var serverURL = UserDefaults.standard.string(forKey: "serverURL") ?? ""
    @State private var apiToken = UserDefaults.standard.string(forKey: "apiToken") ?? ""
    @State private var networkMonitor = NetworkMonitor()

    var body: some Scene {
        WindowGroup {
            Group {
                if let api = makeClient() {
                    ContentView(api: api, serverURL: $serverURL, apiToken: $apiToken)
                } else {
                    SetupView(serverURL: $serverURL, apiToken: $apiToken)
                }
            }
            .environment(networkMonitor)
            .onAppear { networkMonitor.start() }
        }
    }

    private func makeClient() -> APIClient? {
        guard !serverURL.isEmpty, !apiToken.isEmpty,
              let url = URL(string: serverURL) else { return nil }
        return APIClient(baseURL: url, token: apiToken)
    }
}

struct SetupView: View {
    @Binding var serverURL: String
    @Binding var apiToken: String

    @State private var urlInput = ""
    @State private var tokenInput = ""
    @State private var isTesting = false
    @State private var testError: String?
    @State private var showQRScanner = false
    @State private var scannedPayload: QRPayload?

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()

            VStack(spacing: 32) {
                VStack(spacing: 8) {
                    Text("Atomic")
                        .font(.largeTitle)
                        .fontWeight(.bold)
                        .foregroundStyle(Theme.textPrimary)
                    Text("Connect to your server")
                        .font(.subheadline)
                        .foregroundStyle(Theme.textSecondary)
                }

                // QR Code scan button
                Button {
                    showQRScanner = true
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: "qrcode.viewfinder")
                            .font(.title2)
                        Text("Scan QR Code")
                            .fontWeight(.semibold)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                }
                .background(Theme.accent, in: RoundedRectangle(cornerRadius: 12))
                .foregroundStyle(.white)
                .padding(.horizontal, 24)

                // Divider
                HStack(spacing: 12) {
                    Rectangle()
                        .fill(Theme.textSecondary.opacity(0.3))
                        .frame(height: 1)
                    Text("or enter manually")
                        .font(.caption)
                        .foregroundStyle(Theme.textSecondary)
                    Rectangle()
                        .fill(Theme.textSecondary.opacity(0.3))
                        .frame(height: 1)
                }
                .padding(.horizontal, 24)

                VStack(spacing: 16) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Server URL")
                            .font(.caption)
                            .foregroundStyle(Theme.textSecondary)
                        TextField("http://192.168.1.100:8080", text: $urlInput)
                            .textFieldStyle(.plain)
                            .padding(12)
                            .background(Theme.surface, in: RoundedRectangle(cornerRadius: 10))
                            .foregroundStyle(Theme.textPrimary)
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                            .keyboardType(.URL)
                    }

                    VStack(alignment: .leading, spacing: 6) {
                        Text("API Token")
                            .font(.caption)
                            .foregroundStyle(Theme.textSecondary)
                        SecureField("Token", text: $tokenInput)
                            .textFieldStyle(.plain)
                            .padding(12)
                            .background(Theme.surface, in: RoundedRectangle(cornerRadius: 10))
                            .foregroundStyle(Theme.textPrimary)
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                    }
                }
                .padding(.horizontal, 24)

                if let error = testError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .padding(.horizontal, 24)
                }

                Button {
                    Task { await connect() }
                } label: {
                    if isTesting {
                        ProgressView()
                            .tint(.white)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                    } else {
                        Text("Connect")
                            .fontWeight(.semibold)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                    }
                }
                .background(Theme.surface, in: RoundedRectangle(cornerRadius: 12))
                .foregroundStyle(Theme.textPrimary)
                .padding(.horizontal, 24)
                .disabled(urlInput.isEmpty || tokenInput.isEmpty || isTesting)
            }
        }
        .preferredColorScheme(.dark)
        .onAppear {
            urlInput = serverURL
            tokenInput = apiToken
        }
        .sheet(isPresented: $showQRScanner) {
            QRScannerView(scannedPayload: $scannedPayload)
        }
        .onChange(of: scannedPayload?.url) {
            if let payload = scannedPayload {
                urlInput = payload.url
                tokenInput = payload.token
                scannedPayload = nil
                Task { await connect() }
            }
        }
    }

    private func connect() async {
        isTesting = true
        testError = nil

        guard let url = URL(string: urlInput) else {
            testError = "Invalid URL"
            isTesting = false
            return
        }

        let client = APIClient(baseURL: url, token: tokenInput)
        do {
            _ = try await client.listAtoms(limit: 1, offset: 0)
            UserDefaults.standard.set(urlInput, forKey: "serverURL")
            UserDefaults.standard.set(tokenInput, forKey: "apiToken")
            serverURL = urlInput
            apiToken = tokenInput
        } catch {
            testError = "Connection failed: \(error.localizedDescription)"
        }
        isTesting = false
    }
}
