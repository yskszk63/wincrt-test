fn main() {
    windows::build! {
        Windows::Win32::System::Pipes::CreatePipe,
        Windows::Win32::Security::SECURITY_ATTRIBUTES,
        Windows::Win32::Foundation::HANDLE,
        Windows::Win32::Foundation::BOOL,
        Windows::Win32::Foundation::CloseHandle,
    };
}
