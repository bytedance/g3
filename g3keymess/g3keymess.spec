%if 0%{?rhel} > 7
%undefine _debugsource_packages
%define pkgconfig_real pkgconf
%endif

%if 0%{?rhel} == 7
%global debug_package %{nil}
%define pkgconfig_real pkgconfig
%endif

%define build_profile release-lto

Name:           g3keymess
Version:        0.1.0
Release:        1%{?dist}
Summary:        Keyless server for G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, %{pkgconfig_real}
BuildRequires:  openssl-devel,
Requires:       systemd

%description
Keyless server for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
SSL_FEATURE=$(pkg-config --atleast-version 1.1.1 libssl || echo "vendored-openssl")
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $SSL_FEATURE, --package g3keymess
sh %{name}/service/generate_systemd.sh


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3keymess %{buildroot}%{_bindir}/g3keymess
install -m 644 -D %{name}/service/g3keymess@.service %{buildroot}/lib/systemd/system/g3keymess@.service


%files
%{_bindir}/g3keymess
/lib/systemd/system/g3keymess@.service
%license LICENSE
%license LICENSE-BUNDLED


%changelog
* Tue Apr 11 2023 G3keymess Maintainers <g3keymess-maintainers@devel.machine> - 0.1.0-1
- New upstream release
