%if 0%{?rhel} > 7
%undefine _debugsource_packages
%define pkgconfig_real pkgconf
%endif

%if 0%{?rhel} == 7
%global debug_package %{nil}
%define pkgconfig_real pkgconfig
%endif

%define build_profile release-lto

Name:           g3tiles
Version:        0.3.0
Release:        1%{?dist}
Summary:        Generic reverse proxy for G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, %{pkgconfig_real}, capnproto
BuildRequires:  openssl-devel,
BuildRequires:  libtool
Requires:       systemd
Requires:       ca-certificates

%description
Generic reverse proxy for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
SSL_FEATURE=$(sh scripts/package/detect_openssl_feature.sh)
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $SSL_FEATURE, --package g3tiles --package g3tiles-ctl
sh %{name}/service/generate_systemd.sh


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3tiles %{buildroot}%{_bindir}/g3tiles
install -m 755 -D target/%{build_profile}/g3tiles-ctl %{buildroot}%{_bindir}/g3tiles-ctl
install -m 644 -D %{name}/service/g3tiles@.service %{buildroot}/lib/systemd/system/g3tiles@.service


%files
%{_bindir}/g3tiles
%{_bindir}/g3tiles-ctl
/lib/systemd/system/g3tiles@.service
%license LICENSE
%license LICENSE-BUNDLED
%license LICENSE-FOREIGN


%changelog
* Tue Jan 30 2024 G3tiles Maintainers <g3tiles-maintainers@devel.machine> - 0.3.0-1
- New upstream release
