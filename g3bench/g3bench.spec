%if 0%{?rhel} > 7
%undefine _debugsource_packages
%endif

%if 0%{?rhel} == 7
%global debug_package %{nil}
%endif

%define build_profile release-lto

Name:           g3bench
Version:        0.8.0
Release:        1%{?dist}
Summary:        Benchmark tool for G3 Project

License:        Apache-2.0
URL:            https://github.com/bytedance/g3
Source0:        %{name}-%{version}.tar.xz

Requires:       ca-certificates

%description
Benchmark tool for G3 Project


%prep
%autosetup


%build
G3_PACKAGE_VERSION="%{version}-%{release}"
export G3_PACKAGE_VERSION
SSL_FEATURE=$(pkg-config --atleast-version 1.1.1 libssl || echo "vendored-openssl")
cargo build --frozen --offline --profile %{build_profile} --no-default-features --features $SSL_FEATURE, --package g3bench


%install
rm -rf $RPM_BUILD_ROOT
install -m 755 -D target/%{build_profile}/g3bench %{buildroot}%{_bindir}/g3bench


%files
%{_bindir}/g3bench
%license LICENSE
%license LICENSE-BUNDLED
%license LICENSE-FOREIGN


%changelog
* Mon Jun 26 2023 G3bench Maintainers <g3bench-maintainers@devel.machine> - 0.8.0-1
- New upstream release
