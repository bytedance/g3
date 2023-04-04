%if 0%{?rhel} > 7
%undefine _debugsource_packages
%define pkgconfig_real pkgconf
%endif

%if 0%{?rhel} == 7
%global debug_package %{nil}
%define pkgconfig_real pkgconfig
%endif

%define build_profile release-lto

Name:           g3rcgen
Version:        0.3.0
Release:        1%{?dist}
Summary:        TLS Interception Certificate Generator for G3 Project

License:        Apache-2.0
#URL:
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  gcc, make, %{pkgconfig_real}
BuildRequires:  openssl-devel

%description
TLS Interception Certificate Generator for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
SSL_FEATURE=$(pkg-config --atleast-version 1.1.1 libssl || echo "vendored-openssl")
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $SSL_FEATURE, --package g3rcgen --package g3rcgen-one


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3rcgen %{buildroot}%{_bindir}/g3rcgen
install -m 755 -D target/%{build_profile}/g3rcgen-one %{buildroot}%{_bindir}/g3rcgen-one
install -m 644 -D %{name}/service/g3rcgen.service %{buildroot}/lib/systemd/system/g3rcgen.service
install -m 644 -D %{name}/service/g3rcgen.preset %{buildroot}/lib/systemd/system-preset/90-g3rcgen.preset


%files
#%license add-license-file-here
%{_bindir}/g3rcgen
%{_bindir}/g3rcgen-one
/lib/systemd/system/g3rcgen.service
/lib/systemd/system-preset/90-g3rcgen.preset


%changelog
* Tue Jan 10 2023 G3rcgen Maintainers <g3rcgen-maintainers@devel.machine> - 0.3.0-1
- New upstream release
