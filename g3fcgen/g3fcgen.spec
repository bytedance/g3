%if 0%{?rhel} > 7
%undefine _debugsource_packages
%define pkgconfig_real pkgconf
%endif

%if 0%{?rhel} == 7
%global debug_package %{nil}
%define pkgconfig_real pkgconfig
%endif

%define build_profile release-lto

Name:           g3fcgen
Version:        0.8.3
Release:        1%{?dist}
Summary:        Fake Certificate Generator for G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, %{pkgconfig_real}
BuildRequires:  openssl-devel

%description
Fake Certificate Generator for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
SSL_FEATURE=$(sh scripts/package/detect_openssl_feature.sh)
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $SSL_FEATURE, --package g3fcgen


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3fcgen %{buildroot}%{_bindir}/g3fcgen
install -m 644 -D %{name}/service/g3fcgen@.service %{buildroot}/lib/systemd/system/g3fcgen@.service


%files
%{_bindir}/g3fcgen
/lib/systemd/system/g3fcgen@.service
%license LICENSE
%license LICENSE-BUNDLED
%license LICENSE-FOREIGN


%changelog
* Fri Jan 03 2025 G3fcgen Maintainers <g3fcgen-maintainers@devel.machine> - 0.8.3-1
- New upstream release
