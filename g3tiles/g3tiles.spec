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
Version:        0.2.5
Release:        1%{?dist}
Summary:        Generic reverse proxy for G3 Project

License:        ASL 2.0
#URL:
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
SSL_FEATURE=$(pkg-config --atleast-version 1.1.1 libssl || echo "vendored-openssl")
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $SSL_FEATURE, --package g3tiles --package g3tiles-ctl
sh %{name}/service/generate_systemd.sh


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3tiles %{buildroot}%{_bindir}/g3tiles
install -m 755 -D target/%{build_profile}/g3tiles-ctl %{buildroot}%{_bindir}/g3tiles-ctl
install -m 644 -D %{name}/service/g3tiles@.service %{buildroot}/lib/systemd/system/g3tiles@.service


%files
#%license add-license-file-here
%{_bindir}/g3tiles
%{_bindir}/g3tiles-ctl
/lib/systemd/system/g3tiles@.service


%changelog
* Mon Jun 12 2023 G3tiles Maintainers <g3tiles-maintainers@devel.machine> - 0.2.5-1
- New upstream release
