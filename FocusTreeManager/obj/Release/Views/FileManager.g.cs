﻿#pragma checksum "..\..\..\Views\FileManager.xaml" "{8829d00f-11b8-4213-878b-770e8597ac16}" "753E2344A0FFDB6A82C72284D180963599153401492FF38B513B26D3F2A447F7"
//------------------------------------------------------------------------------
// <auto-generated>
//     このコードはツールによって生成されました。
//     ランタイム バージョン:4.0.30319.42000
//
//     このファイルへの変更は、以下の状況下で不正な動作の原因になったり、
//     コードが再生成されるときに損失したりします。
// </auto-generated>
//------------------------------------------------------------------------------

using FocusTreeManager.Views.UserControls;
using MahApps.Metro.Controls;
using MahApps.Metro.Controls.Dialogs;
using System;
using System.Diagnostics;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Ink;
using System.Windows.Input;
using System.Windows.Interactivity;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Media.Effects;
using System.Windows.Media.Imaging;
using System.Windows.Media.Media3D;
using System.Windows.Media.TextFormatting;
using System.Windows.Navigation;
using System.Windows.Shapes;
using System.Windows.Shell;


namespace FocusTreeManager.Views {
    
    
    /// <summary>
    /// FileManager
    /// </summary>
    public partial class FileManager : MahApps.Metro.Controls.MetroWindow, System.Windows.Markup.IComponentConnector {
        
        
        #line 27 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal System.Windows.Controls.Grid MainFileGrid;
        
        #line default
        #line hidden
        
        
        #line 29 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal System.Windows.Controls.ColumnDefinition Column1;
        
        #line default
        #line hidden
        
        
        #line 30 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal System.Windows.Controls.ColumnDefinition Column2;
        
        #line default
        #line hidden
        
        
        #line 33 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal System.Windows.Controls.ListView FileChoiceList;
        
        #line default
        #line hidden
        
        
        #line 45 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal System.Windows.Controls.ListView FileTypeList;
        
        #line default
        #line hidden
        
        
        #line 63 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal System.Windows.Controls.RowDefinition ControlsRow;
        
        #line default
        #line hidden
        
        
        #line 65 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal FocusTreeManager.Views.UserControls.FileEditor FileEditor;
        
        #line default
        #line hidden
        
        
        #line 74 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal System.Windows.Controls.Grid GridErrors;
        
        #line default
        #line hidden
        
        
        #line 81 "..\..\..\Views\FileManager.xaml"
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1823:AvoidUnusedPrivateFields")]
        internal FocusTreeManager.Views.UserControls.TutorialButton TutorialButton;
        
        #line default
        #line hidden
        
        private bool _contentLoaded;
        
        /// <summary>
        /// InitializeComponent
        /// </summary>
        [System.Diagnostics.DebuggerNonUserCodeAttribute()]
        [System.CodeDom.Compiler.GeneratedCodeAttribute("PresentationBuildTasks", "4.0.0.0")]
        public void InitializeComponent() {
            if (_contentLoaded) {
                return;
            }
            _contentLoaded = true;
            System.Uri resourceLocater = new System.Uri("/FocusTreeManager;component/views/filemanager.xaml", System.UriKind.Relative);
            
            #line 1 "..\..\..\Views\FileManager.xaml"
            System.Windows.Application.LoadComponent(this, resourceLocater);
            
            #line default
            #line hidden
        }
        
        [System.Diagnostics.DebuggerNonUserCodeAttribute()]
        [System.CodeDom.Compiler.GeneratedCodeAttribute("PresentationBuildTasks", "4.0.0.0")]
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1811:AvoidUncalledPrivateCode")]
        internal System.Delegate _CreateDelegate(System.Type delegateType, string handler) {
            return System.Delegate.CreateDelegate(delegateType, this, handler);
        }
        
        [System.Diagnostics.DebuggerNonUserCodeAttribute()]
        [System.CodeDom.Compiler.GeneratedCodeAttribute("PresentationBuildTasks", "4.0.0.0")]
        [System.ComponentModel.EditorBrowsableAttribute(System.ComponentModel.EditorBrowsableState.Never)]
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Design", "CA1033:InterfaceMethodsShouldBeCallableByChildTypes")]
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Maintainability", "CA1502:AvoidExcessiveComplexity")]
        [System.Diagnostics.CodeAnalysis.SuppressMessageAttribute("Microsoft.Performance", "CA1800:DoNotCastUnnecessarily")]
        void System.Windows.Markup.IComponentConnector.Connect(int connectionId, object target) {
            switch (connectionId)
            {
            case 1:
            
            #line 10 "..\..\..\Views\FileManager.xaml"
            ((FocusTreeManager.Views.FileManager)(target)).Loaded += new System.Windows.RoutedEventHandler(this.MetroWindow_Loaded);
            
            #line default
            #line hidden
            return;
            case 2:
            this.MainFileGrid = ((System.Windows.Controls.Grid)(target));
            return;
            case 3:
            this.Column1 = ((System.Windows.Controls.ColumnDefinition)(target));
            return;
            case 4:
            this.Column2 = ((System.Windows.Controls.ColumnDefinition)(target));
            return;
            case 5:
            this.FileChoiceList = ((System.Windows.Controls.ListView)(target));
            
            #line 35 "..\..\..\Views\FileManager.xaml"
            this.FileChoiceList.SelectionChanged += new System.Windows.Controls.SelectionChangedEventHandler(this.FileChoiceList_SelectionChanged);
            
            #line default
            #line hidden
            return;
            case 6:
            this.FileTypeList = ((System.Windows.Controls.ListView)(target));
            
            #line 47 "..\..\..\Views\FileManager.xaml"
            this.FileTypeList.SelectionChanged += new System.Windows.Controls.SelectionChangedEventHandler(this.FileTypeList_SelectionChanged);
            
            #line default
            #line hidden
            return;
            case 7:
            this.ControlsRow = ((System.Windows.Controls.RowDefinition)(target));
            return;
            case 8:
            this.FileEditor = ((FocusTreeManager.Views.UserControls.FileEditor)(target));
            return;
            case 9:
            this.GridErrors = ((System.Windows.Controls.Grid)(target));
            return;
            case 10:
            this.TutorialButton = ((FocusTreeManager.Views.UserControls.TutorialButton)(target));
            return;
            }
            this._contentLoaded = true;
        }
    }
}

