
%undefine _debugsource_packages
%define build_profile release-lto

Name:           g3bench
Version:        0.9.7
Release:        1%{?dist}
Summary:        Benchmark tool for G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, pkgconf
BuildRequires:  openssl-devel

Requires:       ca-certificates

%description
G3 Benchmark Tool


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features rustls-ring,quic --package g3bench


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3bench %{buildroot}%{_bindir}/g3bench


%files
%{_bindir}/g3bench
%license LICENSE
%license LICENSE-BUNDLED
%license LICENSE-FOREIGN


%changelog
* Thu Jan 15 2026 G3bench Maintainers <g3bench-maintainers@devel.machine> - 0.9.7-1
- New upstream release
