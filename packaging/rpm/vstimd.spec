Name:           vstimd
Version:        0.1.0
Release:        1%{?dist}
Summary:        Visual stimulus display server for neuroscience experiments
License:        AGPL-3.0-or-later
URL:            https://github.com/braemons/vstimd

# The binary is pre-built; this spec does not compile from source.
# Build with: cargo build --release [--target <triple>]
# Then: rpmbuild -bb packaging/rpm/vstimd.spec \
#           --define "_sourcedir $(pwd)/target/release" \
#           --define "_sysdir $(pwd)/packaging/systemd"

%description
vstimd drives a display directly via VK_KHR_display without a compositor,
providing sub-millisecond frame timing for psychophysics experiments.

Communicates via ZMQ (port 5555) using a protobuf protocol. Supports
Jetson Orin Nano, Raspberry Pi 4/5, and desktop NVIDIA/AMD GPUs.

%pre
getent group vstimd >/dev/null || groupadd -r vstimd
getent passwd vstimd >/dev/null || \
    useradd -r -g vstimd -G input,video \
            -s /sbin/nologin \
            -c "vstimd visual stimulus server" vstimd

%install
install -D -m 0755 %{_sourcedir}/vstimd      %{buildroot}%{_bindir}/vstimd
install -D -m 0644 %{_sysdir}/vstimd.service %{buildroot}%{_unitdir}/vstimd.service

%post
%systemd_post vstimd.service

%preun
%systemd_preun vstimd.service

%postun
%systemd_postun_with_restart vstimd.service

%files
%{_bindir}/vstimd
%{_unitdir}/vstimd.service
